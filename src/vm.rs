use std::ffi::CStr;
use std::io::{Read, Seek, SeekFrom};
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Sub};

use bytemuck::{Pod, cast, from_bytes, from_bytes_mut, pod_read_unaligned};
use byteorder::{LittleEndian, ReadBytesExt};
use num_enum::TryFromPrimitive;

#[derive(Clone, Copy, Debug, TryFromPrimitive)]
#[repr(u8)]
pub enum Opcode {
    Undef,
    Ignore,
    Break,
    Enter,
    Leave,
    Call,
    Push,
    Pop,
    Const,
    Local,
    Jump,
    Eq,
    Ne,
    Lti,
    Lei,
    Gti,
    Gei,
    Ltu,
    Leu,
    Gtu,
    Geu,
    Eqf,
    Nef,
    Ltf,
    Lef,
    Gtf,
    Gef,
    Load1,
    Load2,
    Load4,
    Store1,
    Store2,
    Store4,
    Arg,
    BlockCopy,
    Sex8,
    Sex16,
    Negi,
    Add,
    Sub,
    Divi,
    Divu,
    Modi,
    Modu,
    Muli,
    Mulu,
    Band,
    Bor,
    Bxor,
    Bcom,
    Lsh,
    Rshi,
    Rshu,
    Negf,
    Addf,
    Subf,
    Divf,
    Mulf,
    Cvif,
    Cvfi,
}

#[derive(Debug)]
pub struct Instruction {
    pub opcode: Opcode,
    pub arg: u32,
}

#[derive(Default)]
pub struct Vm {
    pub code: Vec<Instruction>,
    pub data: Vec<u8>,
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
            let opcode = Opcode::try_from_primitive(reader.read_u8()?)?;
            let arg = match opcode {
                Opcode::Enter
                | Opcode::Leave
                | Opcode::Const
                | Opcode::Local
                | Opcode::Eq
                | Opcode::Ne
                | Opcode::Lti
                | Opcode::Lei
                | Opcode::Gti
                | Opcode::Gei
                | Opcode::Ltu
                | Opcode::Leu
                | Opcode::Gtu
                | Opcode::Geu
                | Opcode::Eqf
                | Opcode::Nef
                | Opcode::Ltf
                | Opcode::Lef
                | Opcode::Gtf
                | Opcode::Gef
                | Opcode::BlockCopy => reader.read_u32::<LittleEndian>()?,
                Opcode::Arg => reader.read_u8()?.into(),
                _ => 0,
            };

            self.code.push(Instruction { opcode, arg });
        }

        reader.seek(SeekFrom::Start(data_offset.into()))?;
        self.data.resize(data_length + lit_length + bss_length, 0);
        reader.read_exact(&mut self.data[..data_length + lit_length])?;

        self.pc = 0;
        self.program_stack = self.data.len() as u32;
        self.op_stack.clear();

        Ok(())
    }

    pub fn mem_slice(&self, address: usize, size: usize) -> &[u8] {
        &self.data[address..][..size]
    }

    pub fn mem_slice_mut(&mut self, address: usize, size: usize) -> &mut [u8] {
        &mut self.data[address..][..size]
    }

    pub fn cast_mem<T: Pod>(&self, address: u32) -> &T {
        from_bytes(self.mem_slice(address as usize, size_of::<T>()))
    }

    pub fn cast_mem_mut<T: Pod>(&mut self, address: u32) -> &mut T {
        from_bytes_mut(self.mem_slice_mut(address as usize, size_of::<T>()))
    }

    pub fn read_mem<T: Pod>(&self, address: u32) -> T {
        *self.cast_mem(address)
    }

    pub fn write_mem<T: Pod>(&mut self, address: u32, value: T) {
        *self.cast_mem_mut(address) = value;
    }

    pub fn read_local<T: Pod>(&self, offset: u32) -> T {
        self.read_mem(self.program_stack + offset)
    }

    pub fn read_arg<T: Pod>(&self, n: u32) -> T {
        self.read_local(n * 4 + 8)
    }

    pub fn read_cstr(&self, address: u32) -> &CStr {
        CStr::from_bytes_until_nul(&self.data[address as usize..]).unwrap()
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
            self.write_mem::<u32>(self.program_stack, arg);
        }
        self.program_stack -= 8;
        self.write_mem::<u32>(self.program_stack + 4, old_stack);
        self.write_mem::<u32>(self.program_stack, 0xdeadbeef);
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
            Opcode::Enter => {
                let old_stack = self.program_stack;
                self.program_stack -= arg;
                self.write_mem(self.program_stack + 4, old_stack);
            }
            Opcode::Leave => {
                self.program_stack += arg;
                self.pc = self.read_mem(self.program_stack);
                if self.pc == 0xdeadbeef {
                    self.program_stack = self.read_mem(self.program_stack + 4);
                    return Some(ExitReason::Return);
                }
            }
            Opcode::Call => {
                let pc = self.op_stack.pop().unwrap();
                if (pc as i32) < 0 {
                    return Some(ExitReason::Syscall((-(pc as i32) - 1) as u32));
                } else {
                    self.write_mem(self.program_stack, self.pc);
                    self.pc = pc;
                }
            }
            Opcode::Push => self.op_stack.push(0),
            Opcode::Pop => {
                self.op_stack.pop().unwrap();
            }
            Opcode::Const => self.op_stack.push(arg),
            Opcode::Local => self.op_stack.push(self.program_stack + arg),
            Opcode::Jump => self.pc = self.op_stack.pop().unwrap(),
            Opcode::Eq => self.branch_if(arg, u32::eq),
            Opcode::Ne => self.branch_if(arg, u32::ne),
            Opcode::Lti => self.branch_if(arg, i32::lt),
            Opcode::Lei => self.branch_if(arg, i32::le),
            Opcode::Gti => self.branch_if(arg, i32::gt),
            Opcode::Gei => self.branch_if(arg, i32::ge),
            Opcode::Ltu => self.branch_if(arg, u32::lt),
            Opcode::Leu => self.branch_if(arg, u32::le),
            Opcode::Gtu => self.branch_if(arg, u32::gt),
            Opcode::Geu => self.branch_if(arg, u32::ge),
            Opcode::Eqf => self.branch_if(arg, f32::eq),
            Opcode::Nef => self.branch_if(arg, f32::ne),
            Opcode::Ltf => self.branch_if(arg, f32::lt),
            Opcode::Lef => self.branch_if(arg, f32::le),
            Opcode::Gtf => self.branch_if(arg, f32::gt),
            Opcode::Gef => self.branch_if(arg, f32::ge),
            Opcode::Load1 => {
                let address = self.op_stack.pop().unwrap();
                self.op_stack.push(self.read_mem::<u8>(address) as u32);
            }
            Opcode::Load2 => {
                let address = self.op_stack.pop().unwrap();
                self.op_stack.push(self.read_mem::<u16>(address) as u32);
            }
            Opcode::Load4 => {
                let address = self.op_stack.pop().unwrap();
                // We have to do an unaligned read here because some qvms don't behave
                self.op_stack
                    .push(pod_read_unaligned(self.mem_slice(address as usize, 4)));
            }
            Opcode::Store1 => {
                let value = self.op_stack.pop().unwrap() as u8;
                let address = self.op_stack.pop().unwrap();
                self.write_mem(address, value);
            }
            Opcode::Store2 => {
                let value = self.op_stack.pop().unwrap() as u16;
                let address = self.op_stack.pop().unwrap();
                self.write_mem(address, value);
            }
            Opcode::Store4 => {
                let value = self.op_stack.pop().unwrap();
                let address = self.op_stack.pop().unwrap();
                self.write_mem(address, value);
            }
            Opcode::Arg => {
                let value = self.op_stack.pop().unwrap();
                self.write_mem(self.program_stack + arg, value);
            }
            Opcode::BlockCopy => {
                let src = self.op_stack.pop().unwrap() as usize;
                let dst = self.op_stack.pop().unwrap() as usize;
                self.data.copy_within(src..src + arg as usize, dst);
            }
            Opcode::Sex8 => {
                let value = self.op_stack.pop().unwrap();
                self.op_stack.push(value as i8 as i32 as u32);
            }
            Opcode::Sex16 => {
                let value = self.op_stack.pop().unwrap();
                self.op_stack.push(value as i16 as i32 as u32);
            }
            Opcode::Negi => self.unary_op(i32::wrapping_neg),
            Opcode::Add => self.binary_op(u32::wrapping_add),
            Opcode::Sub => self.binary_op(u32::wrapping_sub),
            Opcode::Divi => self.binary_op(i32::wrapping_div),
            Opcode::Divu => self.binary_op(u32::wrapping_div),
            Opcode::Modi => self.binary_op(i32::wrapping_rem),
            Opcode::Modu => self.binary_op(u32::wrapping_rem),
            Opcode::Muli => self.binary_op(i32::wrapping_mul),
            Opcode::Mulu => self.binary_op(u32::wrapping_mul),
            Opcode::Band => self.binary_op(u32::bitand),
            Opcode::Bor => self.binary_op(u32::bitor),
            Opcode::Bxor => self.binary_op(u32::bitxor),
            Opcode::Bcom => self.unary_op(u32::not),
            Opcode::Lsh => self.binary_op(u32::wrapping_shl),
            Opcode::Rshi => self.binary_op(|a: i32, b: i32| a.wrapping_shr(b as u32)),
            Opcode::Rshu => self.binary_op(u32::wrapping_shr),
            Opcode::Negf => self.unary_op(<f32>::neg),
            Opcode::Addf => self.binary_op(<f32>::add),
            Opcode::Subf => self.binary_op(<f32>::sub),
            Opcode::Divf => self.binary_op(<f32>::div),
            Opcode::Mulf => self.binary_op(<f32>::mul),
            Opcode::Cvif => {
                let value = self.op_stack.pop().unwrap();
                self.op_stack.push(cast(value as i32 as f32));
            }
            Opcode::Cvfi => {
                let value: f32 = cast(self.op_stack.pop().unwrap());
                self.op_stack.push(value as i32 as u32);
            }
            _ => unimplemented!(),
        }

        None
    }
}
