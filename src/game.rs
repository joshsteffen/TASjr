use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
    path::Path,
};

use bytemuck::{Zeroable, cast, cast_slice_mut};
use glam::Vec3;

use crate::{
    Snapshot,
    fs::Fs,
    q3::{
        ENTITYNUM_NONE, ENTITYNUM_WORLD, MAX_CLIENTS, Map, gameExport_t::*, gameImport_t::*,
        playerState_t, qtime_t, sharedEntity_t, sharedTraps_t::*, trace_t, usercmd_t, vmCvar_t,
    },
    vm::{ExitReason, Vm},
};

#[derive(Clone, Default, Debug)]
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

#[derive(Clone, Copy)]
pub struct GameData<T> {
    pub address: u32,
    pub count: u32,
    pub sizeof: u32,
    phantom: PhantomData<T>,
}

impl<T> GameData<T> {
    fn new(address: u32, count: u32, sizeof: u32) -> Self {
        Self {
            address,
            count,
            sizeof,
            phantom: PhantomData,
        }
    }

    fn index_of(&self, address: u32) -> u32 {
        (address - self.address) / self.sizeof
    }

    fn address(&self, index: u32) -> u32 {
        self.address + index * self.sizeof
    }
}

#[derive(Clone)]
pub struct Game {
    pub cvars: Cvars,
    pub vm: Vm,
    pub g_entities: Option<GameData<sharedEntity_t>>,
    pub clients: Option<GameData<playerState_t>>,
    pub init_time: i32,
    pub time: i32,
    usercmd: usercmd_t,
    linked_entities: HashSet<u32>,
}

impl Game {
    pub fn new<P: AsRef<Path>>(fs: &Fs, vm_path: P) -> Self {
        let cvars = Cvars::default();

        let mut vm = Vm::default();
        let f = fs.open(vm_path).unwrap();
        vm.load(f).unwrap();

        Self {
            cvars,
            vm,
            g_entities: None,
            clients: None,
            usercmd: usercmd_t::zeroed(),
            init_time: 0,
            time: 0,
            linked_entities: HashSet::new(),
        }
    }

    pub fn init(&mut self) {
        self.g_init(0, 0, false);
        for _ in 0..3 {
            self.g_run_frame(self.time);
            self.time += 100;
        }
        self.init_time = self.time;
        self.g_client_connect(0, true, false).unwrap();
        self.g_client_begin(0);
    }

    pub fn run_frame(&mut self, usercmd: usercmd_t) {
        self.usercmd = usercmd;
        self.usercmd.serverTime = self.time;

        // We use absolute angles, but the game expects them to be relative to delta_angles
        let ps = self
            .vm
            .memory
            .cast_mut::<playerState_t>(self.clients.unwrap().address);
        (0..3).for_each(|i| self.usercmd.angles[i] -= ps.delta_angles[i]);

        self.g_client_think(0);
        self.g_run_frame(self.time);
        self.time += 8;
    }

    pub fn relative_time(&self) -> i32 {
        self.time - self.init_time
    }

    pub fn frame(&self) -> usize {
        assert!(self.relative_time() % 8 == 0);
        (self.relative_time() / 8) as usize
    }

    pub fn ps(&self) -> &playerState_t {
        self.vm.memory.cast(self.clients.unwrap().address)
    }

    pub fn entity(&self, index: u32) -> &sharedEntity_t {
        self.vm.memory.cast(self.g_entities.unwrap().address(index))
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
            GAME_INIT as _,
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
            GAME_CLIENT_CONNECT as _,
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
            Err(self.vm.memory.cstr(result).to_string_lossy().into())
        } else {
            Ok(())
        }
    }

    pub fn g_client_begin(&mut self, client_num: i32) {
        self.call_vm([
            GAME_CLIENT_BEGIN as _,
            client_num as u32,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]);
    }

    pub fn g_client_think(&mut self, client_num: i32) {
        self.call_vm([
            GAME_CLIENT_THINK as _,
            client_num as u32,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]);
    }

    pub fn g_run_frame(&mut self, level_time: i32) {
        self.call_vm([
            GAME_RUN_FRAME as _,
            level_time as u32,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]);
    }

    fn handle_syscall(&mut self, syscall: u32) {
        match syscall as _ {
            G_PRINT => {
                let s = self.vm.memory.cstr(self.vm.read_arg(0)).to_string_lossy();
                println!("{s}");
                self.vm.set_result(0);
            }
            G_ERROR => {
                let s = self.vm.memory.cstr(self.vm.read_arg(0)).to_string_lossy();
                panic!("{s}");
            }
            G_MILLISECONDS => {
                self.vm.set_result(0);
            }
            G_CVAR_REGISTER => {
                let vm_cvar = self.vm.read_arg::<u32>(0);
                let name = self
                    .vm
                    .memory
                    .cstr(self.vm.read_arg(1))
                    .to_string_lossy()
                    .to_string();
                let default = self
                    .vm
                    .memory
                    .cstr(self.vm.read_arg(2))
                    .to_string_lossy()
                    .to_string();
                let flags = self.vm.read_arg::<u32>(3);
                eprintln!("G_CVAR_REGISTER {name} {default:?} {flags}");
                let handle = self.cvars.register(name.to_owned(), default.to_owned());
                if vm_cvar != 0 {
                    let vm_cvar = self.vm.memory.cast_mut::<vmCvar_t>(vm_cvar);
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
                let vm_cvar = self.vm.memory.cast_mut::<vmCvar_t>(self.vm.read_arg(0));
                let _name = &self.cvars.registered[vm_cvar.handle as usize];
                self.vm.set_result(0);
            }
            G_CVAR_SET => {
                let name = self.vm.memory.cstr(self.vm.read_arg(0)).to_string_lossy();
                let value = self.vm.memory.cstr(self.vm.read_arg(1)).to_string_lossy();
                self.cvars.set(&name, value.to_string());
                self.vm.set_result(0);
            }
            G_CVAR_VARIABLE_INTEGER_VALUE => {
                let name = self.vm.memory.cstr(self.vm.read_arg(0)).to_string_lossy();
                self.vm.set_result(self.cvars.get_i32(&name) as u32);
            }
            G_CVAR_VARIABLE_STRING_BUFFER => {
                let name = self.vm.memory.cstr(self.vm.read_arg(0)).to_string_lossy();
                let buffer = self.vm.read_arg::<u32>(1);
                let _size = self.vm.read_arg::<u32>(2) as usize;
                eprintln!("G_CVAR_VARIABLE_STRING_BUFFER {name}");
                self.vm.memory.write::<u8>(buffer, 0);
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
                self.g_entities = Some(GameData::new(
                    self.vm.read_arg(0),
                    self.vm.read_arg(1),
                    self.vm.read_arg(2),
                ));
                self.clients = Some(GameData::new(
                    self.vm.read_arg(3),
                    MAX_CLIENTS,
                    self.vm.read_arg(4),
                ));
                self.vm.set_result(0);
            }
            G_SEND_SERVER_COMMAND => {
                let client_num = self.vm.read_arg::<i32>(0);
                let text = self.vm.memory.cstr(self.vm.read_arg(1)).to_string_lossy();
                eprintln!("G_SEND_SERVER_COMMAND {client_num} {text}");
                self.vm.set_result(0);
            }
            G_SET_CONFIGSTRING => {
                let num = self.vm.read_arg::<u32>(0);
                let string = self.vm.memory.cstr(self.vm.read_arg(1)).to_string_lossy();
                eprintln!("G_SET_CONFIGSTRING {num} {string}");
                self.vm.set_result(0);
            }
            G_GET_CONFIGSTRING => {
                let num = self.vm.read_arg::<u32>(0);
                let buffer = self.vm.read_arg::<u32>(1);
                let _size = self.vm.read_arg::<u32>(2) as usize;
                self.vm.memory.write::<u8>(buffer, 0);
                eprintln!("G_GET_CONFIGSTRING {num}");
                self.vm.set_result(0);
            }
            G_GET_USERINFO => {
                eprintln!("G_GET_USERINFO");
                self.vm.memory.write::<u8>(self.vm.read_arg(1), 0);
                self.vm.set_result(0);
            }
            G_SET_BRUSH_MODEL => {
                let ent_addr = self.vm.read_arg(0);
                let name = self.vm.memory.cstr(self.vm.read_arg(1)).to_string_lossy();
                let model = Map::instance().inline_model(name[1..].parse().unwrap());
                let ent = self.vm.memory.cast_mut::<sharedEntity_t>(ent_addr);
                Map::instance().model_bounds(model, &mut ent.r.mins, &mut ent.r.maxs);
                ent.s.modelindex = model;
                ent.r.bmodel = 1;
                ent.r.contents = -1;
                self.link_entity(ent_addr);
                self.vm.set_result(0);
            }
            G_TRACE => {
                let results = self.vm.read_arg::<u32>(0);
                let start = self.vm.memory.read::<[f32; 3]>(self.vm.read_arg(1));
                let mins = self.vm.memory.read::<[f32; 3]>(self.vm.read_arg(2));
                let maxs = self.vm.memory.read::<[f32; 3]>(self.vm.read_arg(3));
                let end = self.vm.memory.read::<[f32; 3]>(self.vm.read_arg(4));
                let pass_entity_num = self.vm.read_arg::<i32>(5);
                let content_mask = self.vm.read_arg::<i32>(6);

                let mut clip_trace = trace_t::zeroed();

                Map::instance().box_trace(
                    &mut clip_trace,
                    &start,
                    &end,
                    &mins,
                    &maxs,
                    0,
                    content_mask,
                    false,
                );

                clip_trace.entityNum = if clip_trace.fraction == 1.0 {
                    ENTITYNUM_NONE
                } else {
                    ENTITYNUM_WORLD
                } as i32;

                if clip_trace.fraction == 0.0 {
                    *self.vm.memory.cast_mut::<trace_t>(results) = clip_trace;
                    self.vm.set_result(0);
                    return;
                }

                let box_mins = Vec3::min(Vec3::from(start), Vec3::from(end)) + Vec3::from(mins)
                    - Vec3::splat(1.0);
                let box_maxs = Vec3::max(Vec3::from(start), Vec3::from(end))
                    + Vec3::from(maxs)
                    + Vec3::splat(1.0);

                let mut pass_owner_num = -1;
                if pass_entity_num != ENTITYNUM_NONE as _ {
                    let owner_num = self.entity(pass_entity_num as _).r.ownerNum;
                    if owner_num != ENTITYNUM_NONE as _ {
                        pass_owner_num = owner_num;
                    }
                }

                for n in self.entities_in_box(box_mins, box_maxs) {
                    if clip_trace.allsolid != 0 {
                        *self.vm.memory.cast_mut::<trace_t>(results) = clip_trace;
                        self.vm.set_result(0);
                        return;
                    }

                    let ent = self.entity(n);

                    if pass_entity_num != ENTITYNUM_NONE as _
                        && (n == pass_entity_num as _
                            || ent.r.ownerNum == pass_entity_num
                            || ent.r.ownerNum == pass_owner_num)
                    {
                        continue;
                    }

                    if content_mask & ent.r.contents == 0 {
                        continue;
                    }

                    let clip_handle = if ent.r.bmodel != 0 {
                        Map::instance().inline_model(ent.s.modelindex)
                    } else {
                        Map::instance().temp_box_model(&ent.r.mins, &ent.r.maxs, false)
                    };

                    let origin = ent.r.currentOrigin;
                    let angles = if ent.r.bmodel == 0 {
                        [0.0; 3]
                    } else {
                        ent.r.currentAngles
                    };

                    let mut trace = trace_t::zeroed();
                    Map::instance().transformed_box_trace(
                        &mut trace,
                        &start,
                        &end,
                        &mins,
                        &maxs,
                        clip_handle,
                        content_mask,
                        &origin,
                        &angles,
                        false,
                    );

                    if trace.allsolid != 0 {
                        clip_trace.allsolid = 1;
                        trace.entityNum = ent.s.number;
                    } else if trace.startsolid != 0 {
                        clip_trace.startsolid = 1;
                        trace.entityNum = ent.s.number;
                    }

                    if trace.fraction < clip_trace.fraction {
                        let old_start = clip_trace.startsolid;
                        trace.entityNum = ent.s.number;
                        clip_trace = trace;
                        clip_trace.startsolid |= old_start;
                    }
                }

                *self.vm.memory.cast_mut::<trace_t>(results) = clip_trace;
                self.vm.set_result(0);
            }
            G_POINT_CONTENTS => {
                let p = self.vm.memory.read::<[f32; 3]>(self.vm.read_arg(0));
                self.vm
                    .set_result(Map::instance().point_contents(&p, 0) as u32);
            }
            G_LINKENTITY => {
                self.link_entity(self.vm.read_arg(0));
                self.vm.set_result(0);
            }
            G_UNLINKENTITY => {
                self.linked_entities.remove(&self.vm.read_arg(0));
                self.vm.set_result(0);
            }
            G_ENTITIES_IN_BOX => {
                let mins = self.vm.memory.read::<[f32; 3]>(self.vm.read_arg(0));
                let maxs = self.vm.memory.read::<[f32; 3]>(self.vm.read_arg(1));
                let entity_list = self.vm.read_arg::<u32>(2);
                let max_count = self.vm.read_arg::<u32>(3);

                let entities = self
                    .entities_in_box(mins.into(), maxs.into())
                    .into_iter()
                    .take(max_count as usize);

                self.vm.set_result(entities.len() as u32);

                entities.enumerate().for_each(|(i, ent)| {
                    self.vm.memory.write(entity_list + 4 * i as u32, ent);
                });
            }
            G_ENTITY_CONTACT => {
                let mins = self.vm.memory.read::<[f32; 3]>(self.vm.read_arg(0));
                let maxs = self.vm.memory.read::<[f32; 3]>(self.vm.read_arg(1));
                let ent = self.vm.memory.cast::<sharedEntity_t>(self.vm.read_arg(2));

                let clip_handle = if ent.r.bmodel != 0 {
                    Map::instance().inline_model(ent.s.modelindex)
                } else {
                    Map::instance().temp_box_model(&ent.r.mins, &ent.r.maxs, false)
                };

                let mut trace = trace_t::zeroed();

                Map::instance().transformed_box_trace(
                    &mut trace,
                    &[0.0; 3],
                    &[0.0; 3],
                    &mins,
                    &maxs,
                    clip_handle,
                    -1,
                    &ent.r.currentOrigin,
                    &ent.r.currentAngles,
                    false,
                );

                self.vm.set_result(trace.startsolid);
            }
            G_GET_USERCMD => {
                self.vm.memory.write(self.vm.read_arg(1), self.usercmd);
                self.vm.set_result(0);
            }
            G_GET_ENTITY_TOKEN => {
                if let Some(token) = Map::instance().entity_tokens.next() {
                    let token = token.as_bytes();
                    let buffer = self.vm.read_arg::<u32>(0) as usize;
                    let size = self.vm.read_arg::<u32>(1) as usize;
                    let size = size.min(token.len() + 1);
                    let slice = self.vm.memory.slice_mut(buffer, size);
                    slice[..size - 1].copy_from_slice(&token[..size - 1]);
                    slice[size - 1] = 0;
                    self.vm.set_result(1);
                } else {
                    self.vm.set_result(0);
                }
            }
            G_REAL_TIME => {
                let qtime = self.vm.memory.cast_mut::<qtime_t>(self.vm.read_arg(0));
                *qtime = qtime_t::zeroed();
                self.vm.set_result(0);
            }
            G_SNAPVECTOR => {
                self.vm
                    .memory
                    .cast_mut::<[f32; 3]>(self.vm.read_arg(0))
                    .iter_mut()
                    .for_each(|x| *x = x.round_ties_even());
                self.vm.set_result(0);
            }
            G_CEIL => {
                self.vm.set_result(cast(self.vm.read_arg::<f32>(0).ceil()));
            }
            TRAP_MEMSET => {
                self.vm.memory.memset(
                    self.vm.read_arg(0),
                    self.vm.read_arg(1),
                    self.vm.read_arg(2),
                );
                self.vm.set_result(0);
            }
            TRAP_MEMCPY => {
                self.vm.memory.memcpy(
                    self.vm.read_arg(0),
                    self.vm.read_arg(1),
                    self.vm.read_arg(2),
                );
                self.vm.set_result(0);
            }
            TRAP_SIN => {
                self.vm.set_result(cast(self.vm.read_arg::<f32>(0).sin()));
            }
            TRAP_COS => {
                self.vm.set_result(cast(self.vm.read_arg::<f32>(0).cos()));
            }
            TRAP_ATAN2 => {
                self.vm
                    .set_result(cast(f32::atan2(self.vm.read_arg(0), self.vm.read_arg(1))));
            }
            TRAP_SQRT => {
                self.vm.set_result(cast(self.vm.read_arg::<f32>(0).sqrt()));
            }
            TRAP_STRNCPY => {
                let dst = self.vm.read_arg(0);
                self.vm
                    .memory
                    .strncpy(dst, self.vm.read_arg(1), self.vm.read_arg(2));
                self.vm.set_result(dst);
            }
            _ => unimplemented!("syscall {syscall:?}"),
        };
    }

    fn link_entity(&mut self, ent: u32) {
        self.linked_entities.insert(ent);

        let ent = self.vm.memory.cast_mut::<sharedEntity_t>(ent);

        let origin = Vec3::from(ent.r.currentOrigin);
        let angles = Vec3::from(ent.r.currentAngles);
        let mins = Vec3::from(ent.r.mins);
        let maxs = Vec3::from(ent.r.maxs);

        let (absmin, absmax) = if ent.r.bmodel != 0 && angles != Vec3::ZERO {
            let radius = Vec3::splat(mins.abs().max(maxs.abs()).length());
            (origin - radius, origin + radius)
        } else {
            (origin + mins, origin + maxs)
        };

        ent.r.absmin = (absmin - Vec3::ONE).into();
        ent.r.absmax = (absmax + Vec3::ONE).into();
    }

    fn entities_in_box(&self, mins: Vec3, maxs: Vec3) -> Vec<u32> {
        let g_entities = self.g_entities.unwrap();
        self.linked_entities
            .iter()
            .cloned()
            .filter(|&ent| {
                let ent = self.vm.memory.cast::<sharedEntity_t>(ent);
                maxs.cmpge(ent.r.absmin.into()).all() && mins.cmple(ent.r.absmax.into()).all()
            })
            .map(|ent| g_entities.index_of(ent))
            .collect()
    }
}

pub struct GameSnapshot {
    vm: <Vm as Snapshot>::Snapshot,
    g_entities: Option<GameData<sharedEntity_t>>,
    clients: Option<GameData<playerState_t>>,
    time: i32,
    linked_entities: HashSet<u32>,
}

impl Snapshot for Game {
    type Snapshot = GameSnapshot;

    fn take_snapshot(&self, baseline: Option<&Self::Snapshot>) -> Self::Snapshot {
        Self::Snapshot {
            vm: self.vm.take_snapshot(baseline.map(|b| &b.vm)),
            g_entities: self.g_entities,
            clients: self.clients,
            time: self.time,
            linked_entities: self.linked_entities.clone(),
        }
    }

    fn restore_from_snapshot(&mut self, snapshot: &Self::Snapshot) {
        self.vm.restore_from_snapshot(&snapshot.vm);
        self.g_entities = snapshot.g_entities;
        self.clients = snapshot.clients;
        self.time = snapshot.time;
        self.linked_entities = snapshot.linked_entities.clone();
    }
}
