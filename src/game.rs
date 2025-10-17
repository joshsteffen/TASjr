use std::{collections::HashMap, path::Path};

use bytemuck::{Zeroable, cast, cast_slice_mut};

use crate::{
    fs::Fs,
    q3::{
        CM_BoxTrace, CM_PointContents, ENTITYNUM_NONE, ENTITYNUM_WORLD, gameExport_t::*,
        gameImport_t::*, sharedTraps_t::*, trace_t, usercmd_t, vmCvar_t,
    },
    vm::{ExitReason, Vm},
};

#[derive(Default, Debug)]
pub struct Cvars {
    cvars: HashMap<String, String>,
    registered: Vec<String>,
}

impl Cvars {
    pub fn get_str(&self, name: &str) -> &str {
        self.cvars
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn get_i32(&self, name: &str) -> i32 {
        self.get_str(&name.to_ascii_lowercase())
            .parse()
            .unwrap_or_default()
    }

    pub fn get_f32(&self, name: &str) -> f32 {
        self.get_str(&name.to_ascii_lowercase())
            .parse()
            .unwrap_or_default()
    }

    pub fn set(&mut self, name: &str, value: String) {
        self.cvars.insert(name.to_ascii_lowercase(), value);
    }

    pub fn register(&mut self, name: String, value: String) -> usize {
        let handle = self.registered.len();
        self.registered.push(name.to_ascii_lowercase());
        self.cvars.entry(name.to_ascii_lowercase()).or_insert(value);
        handle
    }
}

pub struct Game {
    pub cvars: Cvars,
    pub vm: Vm,

    pub g_entities: u32,
    pub num_g_entities: u32,
    pub sizeof_g_entity: u32,

    pub clients: u32,
    pub sizeof_game_client: u32,

    // TODO: this can be part of CM_ stuff later
    entity_tokens: Box<dyn Iterator<Item = String>>,

    pub user_cmd: usercmd_t,
}

impl Game {
    pub fn new<P: AsRef<Path>>(fs: &Fs, vm_path: P, entity_tokens: Vec<String>) -> Self {
        let cvars = Cvars::default();

        let mut vm = Vm::default();
        let f = fs.open(vm_path).unwrap();
        vm.load(f).unwrap();

        let entity_tokens = Box::new(entity_tokens.into_iter());

        Self {
            cvars,
            vm,
            entity_tokens,
            g_entities: 0,
            num_g_entities: 0,
            sizeof_g_entity: 0,
            clients: 0,
            sizeof_game_client: 0,
            user_cmd: usercmd_t::zeroed(),
        }
    }

    fn call_vm(&mut self, args: [u32; 10]) -> u32 {
        self.vm.prepare_call(&args);
        loop {
            match self.vm.run() {
                ExitReason::Return => return self.vm.op_stack.pop().unwrap(),
                ExitReason::Syscall(syscall) => self.handle_syscall(syscall),
            }
        }
    }

    pub fn g_init(&mut self, level_time: i32, random_seed: i32, restart: bool) {
        self.call_vm([
            GAME_INIT,
            level_time as u32,
            random_seed as u32,
            restart as u32,
            0,
            0,
            0,
            0,
            0,
            0,
        ]);
    }

    pub fn g_client_connect(
        &mut self,
        client_num: i32,
        first_time: bool,
        is_bot: bool,
    ) -> Result<(), String> {
        let result = self.call_vm([
            GAME_CLIENT_CONNECT,
            client_num as u32,
            first_time as u32,
            is_bot as u32,
            0,
            0,
            0,
            0,
            0,
            0,
        ]);
        if result != 0 {
            Err(self.vm.read_cstr(result).to_string_lossy().into())
        } else {
            Ok(())
        }
    }

    pub fn g_client_begin(&mut self, client_num: i32) {
        self.call_vm([GAME_CLIENT_BEGIN, client_num as u32, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    pub fn g_client_think(&mut self, client_num: i32) {
        self.call_vm([GAME_CLIENT_THINK, client_num as u32, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    pub fn g_run_frame(&mut self, level_time: i32) {
        self.call_vm([GAME_RUN_FRAME, level_time as u32, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    fn handle_syscall(&mut self, syscall: u32) {
        match syscall {
            G_PRINT => {
                let s = self.vm.read_cstr(self.vm.read_arg(0)).to_string_lossy();
                println!("{s}");
                self.vm.set_result(0);
            }
            G_ERROR => {
                let s = self.vm.read_cstr(self.vm.read_arg(0)).to_string_lossy();
                panic!("{s}");
            }
            G_MILLISECONDS => {
                self.vm.set_result(0);
            }
            G_CVAR_REGISTER => {
                let vm_cvar = self.vm.read_arg::<u32>(0);
                let name = self
                    .vm
                    .read_cstr(self.vm.read_arg(1))
                    .to_string_lossy()
                    .to_string();
                let default = self
                    .vm
                    .read_cstr(self.vm.read_arg(2))
                    .to_string_lossy()
                    .to_string();
                let flags = self.vm.read_arg::<u32>(3);
                eprintln!("G_CVAR_REGISTER {name} {default:?} {flags}");
                let handle = self.cvars.register(name.to_owned(), default.to_owned());
                if vm_cvar != 0 {
                    let vm_cvar = self.vm.cast_mem_mut::<vmCvar_t>(vm_cvar);
                    vm_cvar.handle = handle as i32;
                    vm_cvar.value = self.cvars.get_f32(&name);
                    vm_cvar.integer = self.cvars.get_i32(&name);
                    let bytes = self.cvars.get_str(&name).as_bytes();
                    let size = bytes.len().min(vm_cvar.string.len());
                    cast_slice_mut(&mut vm_cvar.string[..size]).copy_from_slice(&bytes[..size]);
                }
                self.vm.set_result(0);
            }
            G_CVAR_UPDATE => {
                let vm_cvar = self.vm.cast_mem_mut::<vmCvar_t>(self.vm.read_arg(0));
                let name = &self.cvars.registered[vm_cvar.handle as usize];
                eprintln!("G_CVAR_UPDATE {name}");
                self.vm.set_result(0);
            }
            G_CVAR_SET => {
                let name = self.vm.read_cstr(self.vm.read_arg(0)).to_string_lossy();
                let value = self.vm.read_cstr(self.vm.read_arg(1)).to_string_lossy();
                eprintln!("G_CVAR_SET {name} {value}");
                self.cvars.set(&name, value.to_string());
                self.vm.set_result(0);
            }
            G_CVAR_VARIABLE_INTEGER_VALUE => {
                let name = self.vm.read_cstr(self.vm.read_arg(0)).to_string_lossy();
                self.vm.set_result(self.cvars.get_i32(&name) as u32);
            }
            G_CVAR_VARIABLE_STRING_BUFFER => {
                let name = self.vm.read_cstr(self.vm.read_arg(0)).to_string_lossy();
                let buffer = self.vm.read_arg::<u32>(1);
                let _size = self.vm.read_arg::<u32>(2) as usize;
                eprintln!("G_CVAR_VARIABLE_STRING_BUFFER {name}");
                self.vm.write_mem::<u8>(buffer, 0);
                self.vm.set_result(0);
            }
            G_FS_FOPEN_FILE => {
                self.vm.set_result(0);
            }
            G_FS_READ => {
                self.vm.set_result(0);
            }
            G_FS_WRITE => {
                self.vm.set_result(0);
            }
            G_FS_FCLOSE_FILE => {
                self.vm.set_result(0);
            }
            G_LOCATE_GAME_DATA => {
                self.g_entities = self.vm.read_arg::<u32>(0);
                self.num_g_entities = self.vm.read_arg::<u32>(1);
                self.sizeof_g_entity = self.vm.read_arg::<u32>(2);
                self.clients = self.vm.read_arg::<u32>(3);
                self.sizeof_game_client = self.vm.read_arg::<u32>(4);
                self.vm.set_result(0);
            }
            G_SEND_SERVER_COMMAND => {
                let client_num = self.vm.read_arg::<i32>(0);
                let text = self.vm.read_cstr(self.vm.read_arg(1)).to_string_lossy();
                eprintln!("G_SEND_SERVER_COMMAND {client_num} {text}");
                self.vm.set_result(0);
            }
            G_SET_CONFIGSTRING => {
                let num = self.vm.read_arg::<u32>(0);
                let string = self.vm.read_cstr(self.vm.read_arg(1)).to_string_lossy();
                eprintln!("G_SET_CONFIGSTRING {num} {string}");
                self.vm.set_result(0);
            }
            G_GET_CONFIGSTRING => {
                let num = self.vm.read_arg::<u32>(0);
                let buffer = self.vm.read_arg::<u32>(1);
                let _size = self.vm.read_arg::<u32>(2) as usize;
                self.vm.write_mem::<u8>(buffer, 0);
                eprintln!("G_GET_CONFIGSTRING {num}");
                self.vm.set_result(0);
            }
            G_GET_USERINFO => {
                eprintln!("G_GET_USERINFO");
                self.vm.write_mem::<u8>(self.vm.read_arg(1), 0);
                self.vm.set_result(0);
            }
            G_SET_BRUSH_MODEL => {
                eprintln!("G_SET_BRUSH_MODEL");
                self.vm.set_result(0);
            }
            G_TRACE => {
                let results = self.vm.read_arg::<u32>(0);
                let start = self.vm.read_mem::<[f32; 3]>(self.vm.read_arg(1));
                let mins = self.vm.read_mem::<[f32; 3]>(self.vm.read_arg(2));
                let maxs = self.vm.read_mem::<[f32; 3]>(self.vm.read_arg(3));
                let end = self.vm.read_mem::<[f32; 3]>(self.vm.read_arg(4));
                let _pass_entity_num = self.vm.read_arg::<i32>(5);
                let content_mask = self.vm.read_arg::<i32>(6);
                let trace = self.vm.cast_mem_mut::<trace_t>(results);
                *trace = trace_t::zeroed();
                unsafe {
                    CM_BoxTrace(
                        trace,
                        start.as_ptr(),
                        end.as_ptr(),
                        mins.as_ptr(),
                        maxs.as_ptr(),
                        0,
                        content_mask,
                        0,
                    );
                }
                trace.entityNum = if trace.fraction == 1.0 {
                    ENTITYNUM_NONE
                } else {
                    ENTITYNUM_WORLD
                } as i32;
                self.vm.set_result(0);
            }
            G_POINT_CONTENTS => {
                eprintln!("G_POINT_CONTENTS");
                let p = self.vm.read_mem::<[f32; 3]>(self.vm.read_arg(0));
                unsafe {
                    self.vm.set_result(CM_PointContents(p.as_ptr(), 0) as u32);
                }
            }
            G_LINKENTITY => {
                eprintln!("G_LINKENTITY");
                self.vm.set_result(0);
            }
            G_UNLINKENTITY => {
                eprintln!("G_UNLINKENTITY");
                self.vm.set_result(0);
            }
            G_ENTITIES_IN_BOX => {
                eprintln!("G_ENTITIES_IN_BOX");
                self.vm.set_result(0);
            }
            G_GET_USERCMD => {
                self.vm.write_mem(self.vm.read_arg(1), self.user_cmd);
                self.vm.set_result(0);
            }
            G_GET_ENTITY_TOKEN => {
                if let Some(token) = self.entity_tokens.next() {
                    let token = token.as_bytes();
                    let buffer = self.vm.read_arg::<u32>(0) as usize;
                    let size = self.vm.read_arg::<u32>(1) as usize;
                    let size = (size - 1).min(token.len());
                    self.vm.data[buffer..][..size].copy_from_slice(&token[..size]);
                    self.vm.data[buffer..][size] = 0;
                    self.vm.set_result(1);
                } else {
                    self.vm.set_result(0);
                }
            }
            G_SNAPVECTOR => {
                self.vm
                    .cast_mem_mut::<[f32; 3]>(self.vm.read_arg(0))
                    .iter_mut()
                    .for_each(|x| *x = x.round_ties_even());
                self.vm.set_result(0);
            }
            G_CEIL => {
                self.vm.set_result(cast(self.vm.read_arg::<f32>(0).ceil()));
            }
            TRAP_MEMSET => {
                let dst = self.vm.read_arg::<u32>(0) as usize;
                let value = self.vm.read_arg::<u8>(1);
                let size = self.vm.read_arg::<u32>(2) as usize;
                self.vm.data[dst..][..size].fill(value);
                self.vm.set_result(0);
            }
            TRAP_MEMCPY => {
                let dst = self.vm.read_arg::<u32>(0) as usize;
                let src = self.vm.read_arg::<u32>(1) as usize;
                let size = self.vm.read_arg::<u32>(2) as usize;
                self.vm.data.copy_within(src..src + size, dst);
                self.vm.set_result(0);
            }
            TRAP_SIN => {
                self.vm.set_result(cast(self.vm.read_arg::<f32>(0).sin()));
            }
            TRAP_COS => {
                self.vm.set_result(cast(self.vm.read_arg::<f32>(0).cos()));
            }
            TRAP_SQRT => {
                self.vm.set_result(cast(self.vm.read_arg::<f32>(0).sqrt()));
            }
            TRAP_STRNCPY => {
                let mut dst = self.vm.read_arg::<u32>(0) as usize;
                let mut src = self.vm.read_arg::<u32>(1) as usize;
                let mut size = self.vm.read_arg::<u32>(2) as usize;
                self.vm.set_result(dst as u32);
                while size != 0 && self.vm.data[src] != 0 {
                    self.vm.data[dst] = self.vm.data[src];
                    src += 1;
                    dst += 1;
                    size -= 1;
                }
                while size != 0 {
                    self.vm.data[dst] = 0;
                    dst += 1;
                    size -= 1;
                }
            }
            _ => unimplemented!("syscall {syscall:?}"),
        };
    }
}
