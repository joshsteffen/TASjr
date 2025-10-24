use std::collections::HashMap;
use std::ffi::CStr;
use std::io::{Read, Seek, SeekFrom};
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Sub};
use std::sync::Arc;

use bit_set::BitSet;
use bytemuck::{Pod, cast, from_bytes, from_bytes_mut, pod_read_unaligned};
use byteorder::{LittleEndian, ReadBytesExt};

use crate::Snapshot;
use crate::q3::opcode_t::{Type as opcode_t, *};

const CHUNK_SIZE: usize = 64;

#[derive(Clone, Debug)]
pub struct Instruction {
    pub opcode: opcode_t,
    pub arg: u32,
}

#[derive(Clone, Default)]
pub struct Memory {
    data: Vec<u8>,
    dirty: BitSet,
}

impl Memory {
    pub fn new(mut data: Vec<u8>) -> Self {
        data.resize(data.len().next_multiple_of(CHUNK_SIZE), 0);
        let dirty = BitSet::with_capacity(data.len() / CHUNK_SIZE);
        Self { data, dirty }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn clear_dirty(&mut self) {
        self.dirty.clear();
    }

    pub fn set_dirty(&mut self, address: usize, size: usize) {
        let (start, end) = (address / CHUNK_SIZE, (address + size).div_ceil(CHUNK_SIZE));
        for chunk in start..end {
            self.dirty.insert(chunk);
        }
    }

    pub fn slice(&self, address: usize, size: usize) -> &[u8] {
        &self.data[address..][..size]
    }

    pub fn slice_mut(&mut self, address: usize, size: usize) -> &mut [u8] {
        self.set_dirty(address, size);
        &mut self.data[address..][..size]
    }

    pub fn cast<T: Pod>(&self, address: u32) -> &T {
        from_bytes(self.slice(address as usize, size_of::<T>()))
    }

    pub fn cast_mut<T: Pod>(&mut self, address: u32) -> &mut T {
        from_bytes_mut(self.slice_mut(address as usize, size_of::<T>()))
    }

    pub fn read<T: Pod>(&self, address: u32) -> T {
        *self.cast(address)
    }

    pub fn write<T: Pod>(&mut self, address: u32, value: T) {
        *self.cast_mut(address) = value;
    }

    pub fn cstr(&self, address: u32) -> &CStr {
        CStr::from_bytes_until_nul(&self.data[address as usize..]).unwrap()
    }

    pub fn memset(&mut self, dst: u32, value: u8, size: u32) {
        let (dst, size) = (dst as usize, size as usize);
        self.slice_mut(dst, size).fill(value);
    }

    pub fn memcpy(&mut self, dst: u32, src: u32, size: u32) {
        let (dst, src, size) = (dst as usize, src as usize, size as usize);
        self.set_dirty(dst, size);
        self.data.copy_within(src..src + size, dst);
    }

    pub fn strncpy(&mut self, dst: u32, src: u32, size: u32) {
        let (mut dst, mut src, mut size) = (dst as usize, src as usize, size as usize);
        self.set_dirty(dst, size);
        while size != 0 && self.data[src] != 0 {
            self.data[dst] = self.data[src];
            src += 1;
            dst += 1;
            size -= 1;
        }
        while size != 0 {
            self.data[dst] = 0;
            dst += 1;
            size -= 1;
        }
    }
}

pub enum MemorySnapshot {
    Baseline(Vec<u8>),
    Delta {
        baseline: Arc<Self>,
        chunks: HashMap<usize, Vec<u8>>,
    },
}

impl Snapshot for Memory {
    type Snapshot = Arc<MemorySnapshot>;

    fn take_snapshot(&self, baseline: Option<&Self::Snapshot>) -> Self::Snapshot {
        if let Some(base_snap) = baseline
            && let MemorySnapshot::Baseline(base_mem) = &**base_snap
        {
            Arc::new(MemorySnapshot::Delta {
                baseline: Arc::clone(base_snap),
                chunks: self
                    .dirty
                    .iter()
                    .filter_map(|chunk| {
                        let addr = chunk * CHUNK_SIZE;
                        let data = &self.data[addr..][..CHUNK_SIZE];
                        if data != &base_mem[addr..][..CHUNK_SIZE] {
                            Some((addr, data.to_owned()))
                        } else {
                            None
                        }
                    })
                    .collect(),
            })
        } else {
            Arc::new(MemorySnapshot::Baseline(self.data.clone()))
        }
    }

    fn restore_from_snapshot(&mut self, snapshot: &Self::Snapshot) {
        match &**snapshot {
            MemorySnapshot::Baseline(baseline) => {
                for chunk in &self.dirty {
                    let addr = chunk * CHUNK_SIZE;
                    self.data[addr..][..CHUNK_SIZE]
                        .copy_from_slice(&baseline[addr..][..CHUNK_SIZE]);
                }
            }
            MemorySnapshot::Delta { baseline, chunks } => {
                self.restore_from_snapshot(baseline);
                for (&addr, data) in chunks.iter() {
                    self.data[addr..][..CHUNK_SIZE].copy_from_slice(data);
                }
            }
        }
    }
}

#[derive(Clone, Default)]
pub struct Vm {
    pub code: Vec<Instruction>,
    pub memory: Memory,
    pub pc: u32,
    pub program_stack: u32,
    pub op_stack: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
pub enum ExitReason {
    Return,
    Syscall(u32),
}

impl Vm {
    pub fn load(&mut self, mut reader: impl Read + Seek) -> Result<(), Box<dyn std::error::Error>> {
        let _magic = reader.read_u32::<LittleEndian>()?;
        let instruction_count = reader.read_u32::<LittleEndian>()?;
        let code_offset = reader.read_u32::<LittleEndian>()?;
        let _code_length = reader.read_u32::<LittleEndian>()?;
        let data_offset = reader.read_u32::<LittleEndian>()?;
        let data_length = reader.read_u32::<LittleEndian>()? as usize;
        let lit_length = reader.read_u32::<LittleEndian>()? as usize;
        let bss_length = reader.read_u32::<LittleEndian>()? as usize;

        reader.seek(SeekFrom::Start(code_offset.into()))?;
        self.code.clear();
        for _ in 0..instruction_count {
            let opcode = reader.read_u8()? as opcode_t;
            let arg = match opcode {
                OP_ENTER | OP_LEAVE | OP_CONST | OP_LOCAL | OP_EQ | OP_NE | OP_LTI | OP_LEI
                | OP_GTI | OP_GEI | OP_LTU | OP_LEU | OP_GTU | OP_GEU | OP_EQF | OP_NEF
                | OP_LTF | OP_LEF | OP_GTF | OP_GEF | OP_BLOCK_COPY => {
                    reader.read_u32::<LittleEndian>()?
                }
                OP_ARG => reader.read_u8()?.into(),
                _ => 0,
            };

            self.code.push(Instruction { opcode, arg });
        }

        reader.seek(SeekFrom::Start(data_offset.into()))?;
        let mut data = vec![0; data_length + lit_length + bss_length];
        reader.read_exact(&mut data[..data_length + lit_length])?;

        self.pc = 0;
        self.program_stack = data.len() as u32;
        self.op_stack.clear();
        self.memory = Memory::new(data);

        Ok(())
    }

    pub fn read_local<T: Pod>(&self, offset: u32) -> T {
        self.memory.read(self.program_stack + offset)
    }

    pub fn read_arg<T: Pod>(&self, n: u32) -> T {
        self.read_local(n * 4 + 8)
    }

    fn branch_if<F, T>(&mut self, target: u32, f: F)
    where
        F: Fn(&T, &T) -> bool,
        T: Pod,
    {
        let b = cast(self.op_stack.pop().unwrap());
        let a = cast(self.op_stack.pop().unwrap());
        if f(&a, &b) {
            self.pc = target;
        }
    }

    fn unary_op<F, T>(&mut self, f: F)
    where
        F: Fn(T) -> T,
        T: Pod,
    {
        let x = cast(self.op_stack.pop().unwrap());
        self.op_stack.push(cast(f(x)));
    }

    fn binary_op<F, T>(&mut self, f: F)
    where
        F: Fn(T, T) -> T,
        T: Pod,
    {
        let b = cast(self.op_stack.pop().unwrap());
        let a = cast(self.op_stack.pop().unwrap());
        self.op_stack.push(cast(f(a, b)));
    }

    pub fn prepare_call(&mut self, args: &[u32]) {
        let old_stack = self.program_stack;
        for &arg in args.iter().rev() {
            self.program_stack -= 4;
            self.memory.write::<u32>(self.program_stack, arg);
        }
        self.program_stack -= 8;
        self.memory.write::<u32>(self.program_stack + 4, old_stack);
        self.memory.write::<u32>(self.program_stack, 0xdeadbeef);
        self.pc = 0;
    }

    pub fn set_result(&mut self, result: u32) {
        self.op_stack.push(result);
    }

    pub fn run(&mut self) -> ExitReason {
        loop {
            if let Some(exit_reason) = self.step() {
                return exit_reason;
            }
        }
    }

    pub fn step(&mut self) -> Option<ExitReason> {
        let &Instruction { opcode, arg } = &self.code[self.pc as usize];
        // println!("{}: {opcode:?} {arg:#x}", self.pc);
        self.pc += 1;
        match opcode {
            OP_ENTER => {
                let old_stack = self.program_stack;
                self.program_stack -= arg;
                self.memory.write(self.program_stack + 4, old_stack);
            }
            OP_LEAVE => {
                self.program_stack += arg;
                self.pc = self.memory.read(self.program_stack);
                if self.pc == 0xdeadbeef {
                    self.program_stack = self.memory.read(self.program_stack + 4);
                    return Some(ExitReason::Return);
                }
            }
            OP_CALL => {
                let pc = self.op_stack.pop().unwrap();
                if (pc as i32) < 0 {
                    return Some(ExitReason::Syscall((-(pc as i32) - 1) as u32));
                } else {
                    self.memory.write(self.program_stack, self.pc);
                    self.pc = pc;
                }
            }
            OP_PUSH => self.op_stack.push(0),
            OP_POP => {
                self.op_stack.pop().unwrap();
            }
            OP_CONST => self.op_stack.push(arg),
            OP_LOCAL => self.op_stack.push(self.program_stack + arg),
            OP_JUMP => self.pc = self.op_stack.pop().unwrap(),
            OP_EQ => self.branch_if(arg, u32::eq),
            OP_NE => self.branch_if(arg, u32::ne),
            OP_LTI => self.branch_if(arg, i32::lt),
            OP_LEI => self.branch_if(arg, i32::le),
            OP_GTI => self.branch_if(arg, i32::gt),
            OP_GEI => self.branch_if(arg, i32::ge),
            OP_LTU => self.branch_if(arg, u32::lt),
            OP_LEU => self.branch_if(arg, u32::le),
            OP_GTU => self.branch_if(arg, u32::gt),
            OP_GEU => self.branch_if(arg, u32::ge),
            OP_EQF => self.branch_if(arg, f32::eq),
            OP_NEF => self.branch_if(arg, f32::ne),
            OP_LTF => self.branch_if(arg, f32::lt),
            OP_LEF => self.branch_if(arg, f32::le),
            OP_GTF => self.branch_if(arg, f32::gt),
            OP_GEF => self.branch_if(arg, f32::ge),
            OP_LOAD1 => {
                let address = self.op_stack.pop().unwrap();
                self.op_stack.push(self.memory.read::<u8>(address) as u32);
            }
            OP_LOAD2 => {
                let address = self.op_stack.pop().unwrap();
                self.op_stack.push(self.memory.read::<u16>(address) as u32);
            }
            OP_LOAD4 => {
                let address = self.op_stack.pop().unwrap();
                // We have to do an unaligned read here because some qvms don't behave
                self.op_stack
                    .push(pod_read_unaligned(self.memory.slice(address as usize, 4)));
            }
            OP_STORE1 => {
                let value = self.op_stack.pop().unwrap() as u8;
                let address = self.op_stack.pop().unwrap();
                self.memory.write(address, value);
            }
            OP_STORE2 => {
                let value = self.op_stack.pop().unwrap() as u16;
                let address = self.op_stack.pop().unwrap();
                self.memory.write(address, value);
            }
            OP_STORE4 => {
                let value = self.op_stack.pop().unwrap();
                let address = self.op_stack.pop().unwrap();
                self.memory.write(address, value);
            }
            OP_ARG => {
                let value = self.op_stack.pop().unwrap();
                self.memory.write(self.program_stack + arg, value);
            }
            OP_BLOCK_COPY => {
                let src = self.op_stack.pop().unwrap();
                let dst = self.op_stack.pop().unwrap();
                self.memory.memcpy(dst, src, arg);
            }
            OP_SEX8 => {
                let value = self.op_stack.pop().unwrap();
                self.op_stack.push(value as i8 as i32 as u32);
            }
            OP_SEX16 => {
                let value = self.op_stack.pop().unwrap();
                self.op_stack.push(value as i16 as i32 as u32);
            }
            OP_NEGI => self.unary_op(i32::wrapping_neg),
            OP_ADD => self.binary_op(u32::wrapping_add),
            OP_SUB => self.binary_op(u32::wrapping_sub),
            OP_DIVI => self.binary_op(i32::wrapping_div),
            OP_DIVU => self.binary_op(u32::wrapping_div),
            OP_MODI => self.binary_op(i32::wrapping_rem),
            OP_MODU => self.binary_op(u32::wrapping_rem),
            OP_MULI => self.binary_op(i32::wrapping_mul),
            OP_MULU => self.binary_op(u32::wrapping_mul),
            OP_BAND => self.binary_op(u32::bitand),
            OP_BOR => self.binary_op(u32::bitor),
            OP_BXOR => self.binary_op(u32::bitxor),
            OP_BCOM => self.unary_op(u32::not),
            OP_LSH => self.binary_op(u32::wrapping_shl),
            OP_RSHI => self.binary_op(|a: i32, b: i32| a.wrapping_shr(b as u32)),
            OP_RSHU => self.binary_op(u32::wrapping_shr),
            OP_NEGF => self.unary_op(<f32>::neg),
            OP_ADDF => self.binary_op(<f32>::add),
            OP_SUBF => self.binary_op(<f32>::sub),
            OP_DIVF => self.binary_op(<f32>::div),
            OP_MULF => self.binary_op(<f32>::mul),
            OP_CVIF => {
                let value = self.op_stack.pop().unwrap();
                self.op_stack.push(cast(value as i32 as f32));
            }
            OP_CVFI => {
                let value: f32 = cast(self.op_stack.pop().unwrap());
                self.op_stack.push(value as i32 as u32);
            }
            _ => unimplemented!(),
        }

        None
    }
}

// At least for now the only thing that needs to be snapshotted is the memory.
impl Snapshot for Vm {
    type Snapshot = <Memory as Snapshot>::Snapshot;

    fn take_snapshot(&self, baseline: Option<&Self::Snapshot>) -> Self::Snapshot {
        self.memory.take_snapshot(baseline)
    }

    fn restore_from_snapshot(&mut self, snapshot: &Self::Snapshot) {
        self.memory.restore_from_snapshot(snapshot);
    }
}
