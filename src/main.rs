use std::{collections::HashMap, env::args, fs::File, path::Path};

use bytemuck::{Pod, Zeroable, cast};
use num_enum::TryFromPrimitive;
use qvm::{ExitReason, Vm};

#[derive(Clone, Copy, Debug)]
#[repr(u32)]
#[allow(unused)]
enum GameExport {
    Init,
    Shutdown,
    ClientConnect,
    ClientBegin,
    ClientUserinfoChanged,
    ClientDisconnect,
    ClientCommand,
    ClientThink,
    RunFrame,
    ConsoleCommand,
}

#[derive(Clone, Copy, Debug, TryFromPrimitive)]
#[repr(u32)]
enum Syscall {
    Print,
    Error,
    Milliseconds,
    CvarRegister,
    CvarUpdate,
    CvarSet,
    CvarVariableIntegerValue,
    CvarVariableStringBuffer,
    Argc,
    Argv,
    FsFopenFile,
    FsRead,
    FsWrite,
    FsFcloseFile,
    SendConsoleCommand,
    LocateGameData,
    DropClient,
    SendServerCommand,
    SetConfigString,
    GetConfigString,
    GetUserInfo,
    SetUserInfo,
    GetServerInfo,
    SetBrushModel,
    Trace,
    PointContents,
    InPvs,
    InPvsIgnorePortals,
    AdjustAreaPortalState,
    AreasConnected,
    LinkEntity,
    UnlinkEntity,
    EntitiesInBox,
    EntityContact,
    BotAllocateClient,
    BotFreeClient,
    GetUserCmd,
    GetEntityToken,
    FsGetfileList,
    DebugPolygonCreate,
    DebugPolygonDelete,
    RealTime,
    SnapVector,
    TraceCapsule,
    EntityContactCapsule,
    FsSeek,

    Memset = 100,
    Memcpy,
    Strncpy,
    Sin,
    Cos,
    Atan2,
    Sqrt,
    MatrixMultiply,
    AngleVectors,
    PerpendicularVector,
    Floor,
    Ceil,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
struct VmCvar {
    handle: i32,
    modification_count: i32,
    value: f32,
    integer: i32,
    string: [u8; 256],
}

#[derive(Default, Debug)]
struct Cvars {
    cvars: HashMap<String, String>,
    registered: Vec<String>,
}

impl Cvars {
    fn get_str(&self, name: &str) -> &str {
        self.cvars.get(name).map(String::as_str).unwrap_or("")
    }

    fn get_i32(&self, name: &str) -> i32 {
        self.get_str(name).parse().unwrap_or_default()
    }

    fn get_f32(&self, name: &str) -> f32 {
        self.get_str(name).parse().unwrap_or_default()
    }

    fn set(&mut self, name: &str, value: String) {
        self.cvars.insert(name.to_string(), value);
    }

    fn register(&mut self, name: String, value: String) -> usize {
        let handle = self.registered.len();
        self.registered.push(name.to_owned());
        self.cvars.insert(name, value);
        handle
    }
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
struct Trajectory {
    ty_type: i32,
    tr_time: i32,
    tr_duration: i32,
    tr_base: [f32; 3],
    tr_delta: [f32; 3],
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
struct EntityState {
    number: i32,
    e_type: i32,
    e_flags: i32,
    pos: Trajectory,
    apos: Trajectory,
    time: i32,
    time2: i32,
    origin: [f32; 3],
    origin2: [f32; 3],
    angles: [f32; 3],
    angles2: [f32; 3],
    other_entity_num: i32,
    other_entity_num2: i32,
    ground_entity_num: i32,
    constant_light: i32,
    loop_sound: i32,
    model_index: i32,
    model_index2: i32,
    client_num: i32,
    frame: i32,
    solid: i32,
    event: i32,
    event_parm: i32,
    powerups: i32,
    weapon: i32,
    legs_anim: i32,
    torso_anim: i32,
    generic1: i32,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
struct EntityShared {
    s: EntityState,
    linked: i32,
    link_count: i32,
    sv_flags: i32,
    single_client: i32,
    bmodel: i32,
    mins: [f32; 3],
    maxs: [f32; 3],
    contents: i32,
    absmin: [f32; 3],
    absmax: [f32; 3],
    current_origin: [f32; 3],
    current_angles: [f32; 3],
    owner_num: i32,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
struct SharedEntity {
    s: EntityState,
    r: EntityShared,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
struct PlayerState {
    command_time: i32,
    pm_type: i32,
    bob_cycle: i32,
    pm_flags: i32,
    pm_time: i32,
    origin: [f32; 3],
    velocity: [f32; 3],
    weapon_time: i32,
    gravity: i32,
    speed: i32,
    delta_angles: [i32; 3],
    ground_entity_num: i32,
    legs_timer: i32,
    legs_anim: i32,
    torso_timer: i32,
    torso_anim: i32,
    movement_dir: i32,
    grapple_point: [f32; 3],
    e_flags: i32,
    event_sequence: i32,
    events: [i32; 2],
    event_parms: [i32; 2],
    external_event: i32,
    external_event_parm: i32,
    external_event_time: i32,
    client_num: i32,
    weapon: i32,
    weapon_state: i32,
    view_angles: [f32; 3],
    view_height: i32,
    damage_event: i32,
    damage_yaw: i32,
    damage_pitch: i32,
    damage_count: i32,
    stats: [i32; 16],
    persistant: [i32; 16],
    powerups: [i32; 16],
    ammo: [i32; 16],
    generic1: i32,
    loop_sound: i32,
    jumppad_ent: i32,
    ping: i32,
    pmove_framecount: i32,
    jumppad_frame: i32,
    entity_event_sequence: i32,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
struct UserCmd {
    server_time: i32,
    angles: [i32; 3],
    buttons: i32,
    weapon: u8,
    forward_move: i8,
    right_move: i8,
    up_move: i8,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
struct CPlane {
    normal: [f32; 3],
    dist: f32,
    type_: u8,
    sign_bits: u8,
    pad: [u8; 2],
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
struct Trace {
    all_solid: i32,
    start_solid: i32,
    fraction: f32,
    end_pos: [f32; 3],
    plane: CPlane,
    surface_flags: i32,
    contents: i32,
    entity_num: i32,
}

struct Game {
    cvars: Cvars,
    vm: Vm,

    g_entities: u32,
    num_g_entities: u32,
    sizeof_g_entity: u32,

    clients: u32,
    sizeof_game_client: u32,

    // TODO: this can be part of CM_ stuff later
    entity_tokens: Box<dyn Iterator<Item = &'static str>>,

    user_cmd: UserCmd,
}

impl Game {
    fn new<P: AsRef<Path>>(vm_path: P) -> Self {
        let cvars = Cvars::default();

        let mut vm = Vm::default();
        let f = File::open(vm_path).unwrap();
        vm.load(f).unwrap();

        let entity_tokens = Box::new(
            [
                "{",
                "classname",
                "worldspawn",
                "}",
                "{",
                "classname",
                "info_player_start",
                "}",
            ]
            .into_iter(),
        );

        Self {
            cvars,
            vm,
            entity_tokens,
            g_entities: 0,
            num_g_entities: 0,
            sizeof_g_entity: 0,
            clients: 0,
            sizeof_game_client: 0,
            user_cmd: UserCmd::zeroed(),
        }
    }

    fn call_vm(&mut self, args: [u32; 10]) -> u32 {
        self.vm.prepare_call(&args);
        loop {
            match self.vm.run() {
                ExitReason::Return => return self.vm.op_stack.pop().unwrap(),
                ExitReason::Syscall(syscall) => {
                    self.handle_syscall(Syscall::try_from_primitive(syscall).unwrap())
                }
            }
        }
    }

    fn g_init(&mut self, level_time: i32, random_seed: i32, restart: bool) {
        self.call_vm([
            GameExport::Init as u32,
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

    fn g_client_connect(
        &mut self,
        client_num: i32,
        first_time: bool,
        is_bot: bool,
    ) -> Result<(), String> {
        let result = self.call_vm([
            GameExport::ClientConnect as u32,
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

    fn g_client_begin(&mut self, client_num: i32) {
        self.call_vm([
            GameExport::ClientBegin as u32,
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

    fn g_client_think(&mut self, client_num: i32) {
        self.call_vm([
            GameExport::ClientThink as u32,
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

    fn g_run_frame(&mut self, level_time: i32) {
        self.call_vm([
            GameExport::RunFrame as u32,
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

    fn handle_syscall(&mut self, syscall: Syscall) {
        match syscall {
            Syscall::Print => {
                let s = self.vm.read_cstr(self.vm.read_arg(0)).to_string_lossy();
                println!("{s}");
                self.vm.set_result(0);
            }
            Syscall::Error => {
                let s = self.vm.read_cstr(self.vm.read_arg(0)).to_string_lossy();
                panic!("{s}");
            }
            Syscall::Milliseconds => {
                self.vm.set_result(0);
            }
            Syscall::CvarRegister => {
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
                eprintln!("CvarRegister {name} {default:?} {flags}");
                self.cvars.set(&name, default.to_string());
                let handle = self.cvars.register(name.to_owned(), default.to_owned());
                if vm_cvar != 0 {
                    let vm_cvar = self.vm.cast_mem_mut::<VmCvar>(vm_cvar);
                    vm_cvar.handle = handle as i32;
                    vm_cvar.value = self.cvars.get_f32(&name);
                    vm_cvar.integer = self.cvars.get_i32(&name);
                    let bytes = self.cvars.get_str(&name).as_bytes();
                    let size = bytes.len().min(vm_cvar.string.len());
                    vm_cvar.string[..size].copy_from_slice(&bytes[..size]);
                }
                self.vm.set_result(0);
            }
            Syscall::CvarUpdate => {
                let vm_cvar = self.vm.cast_mem_mut::<VmCvar>(self.vm.read_arg(0));
                let name = &self.cvars.registered[vm_cvar.handle as usize];
                eprintln!("CvarUpdate {name}");
                self.vm.set_result(0);
            }
            Syscall::CvarSet => {
                let name = self.vm.read_cstr(self.vm.read_arg(0)).to_string_lossy();
                let value = self.vm.read_cstr(self.vm.read_arg(1)).to_string_lossy();
                eprintln!("CvarSet {name} {value}");
                self.cvars.set(&name, value.to_string());
                self.vm.set_result(0);
            }
            Syscall::CvarVariableIntegerValue => {
                let name = self.vm.read_cstr(self.vm.read_arg(0)).to_string_lossy();
                self.vm.set_result(self.cvars.get_i32(&name) as u32);
            }
            Syscall::CvarVariableStringBuffer => {
                let name = self.vm.read_cstr(self.vm.read_arg(0)).to_string_lossy();
                let buffer = self.vm.read_arg::<u32>(1);
                let _size = self.vm.read_arg::<u32>(2) as usize;
                eprintln!("CvarVariableStringBuffer {name}");
                self.vm.write_mem::<u8>(buffer, 0);
                self.vm.set_result(0);
            }
            Syscall::FsFopenFile => {
                self.vm.set_result(0);
            }
            Syscall::FsRead => {
                self.vm.set_result(0);
            }
            Syscall::FsWrite => {
                self.vm.set_result(0);
            }
            Syscall::FsFcloseFile => {
                self.vm.set_result(0);
            }
            Syscall::LocateGameData => {
                self.g_entities = self.vm.read_arg::<u32>(0);
                self.num_g_entities = self.vm.read_arg::<u32>(1);
                self.sizeof_g_entity = self.vm.read_arg::<u32>(2);
                self.clients = self.vm.read_arg::<u32>(3);
                self.sizeof_game_client = self.vm.read_arg::<u32>(4);
                self.vm.set_result(0);
            }
            Syscall::SendServerCommand => {
                let client_num = self.vm.read_arg::<i32>(0);
                let text = self.vm.read_cstr(self.vm.read_arg(1)).to_string_lossy();
                eprintln!("SendServerCommand {client_num} {text}");
                self.vm.set_result(0);
            }
            Syscall::SetConfigString => {
                let num = self.vm.read_arg::<u32>(0);
                let string = self.vm.read_cstr(self.vm.read_arg(1)).to_string_lossy();
                eprintln!("SetConfigString {num} {string}");
                self.vm.set_result(0);
            }
            Syscall::GetConfigString => {
                let num = self.vm.read_arg::<u32>(0);
                let buffer = self.vm.read_arg::<u32>(1);
                let _size = self.vm.read_arg::<u32>(2) as usize;
                self.vm.write_mem::<u8>(buffer, 0);
                eprintln!("GetConfigString {num}");
                self.vm.set_result(0);
            }
            Syscall::GetUserInfo => {
                eprintln!("GetUserInfo");
                self.vm.write_mem::<u8>(self.vm.read_arg(1), 0);
                self.vm.set_result(0);
            }
            Syscall::Trace => {
                let results = self.vm.read_arg::<u32>(0);
                let start = self.vm.read_mem::<[f32; 3]>(self.vm.read_arg(1));
                let mins = self.vm.read_mem::<[f32; 3]>(self.vm.read_arg(2));
                let maxs = self.vm.read_mem::<[f32; 3]>(self.vm.read_arg(3));
                let end = self.vm.read_mem::<[f32; 3]>(self.vm.read_arg(4));
                let pass_entity_num = self.vm.read_arg::<i32>(5);
                let content_mask = self.vm.read_arg::<i32>(6);
                let capsule = self.vm.read_arg::<i32>(7);
                let trace = self.vm.cast_mem_mut::<Trace>(results);
                *trace = Trace::zeroed();
                trace.fraction = 1.0;
                trace.end_pos = end;
                eprintln!(
                    "Trace {results} {start:?} {mins:?} {maxs:?} {end:?} {pass_entity_num} {content_mask} {capsule}"
                );
                self.vm.set_result(0);
            }
            Syscall::PointContents => {
                eprintln!("PointContents");
                self.vm.set_result(0);
            }
            Syscall::LinkEntity => {
                eprintln!("LinkEntity");
                self.vm.set_result(0);
            }
            Syscall::UnlinkEntity => {
                eprintln!("UnlinkEntity");
                self.vm.set_result(0);
            }
            Syscall::EntitiesInBox => {
                eprintln!("EntitiesInBox");
                self.vm.set_result(0);
            }
            Syscall::GetUserCmd => {
                self.vm.write_mem(self.vm.read_arg(1), self.user_cmd);
                self.vm.set_result(0);
            }
            Syscall::GetEntityToken => {
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
            Syscall::SnapVector => {
                self.vm
                    .cast_mem_mut::<[f32; 3]>(self.vm.read_arg(0))
                    .iter_mut()
                    .for_each(|x| *x = x.round_ties_even());
                self.vm.set_result(0);
            }
            Syscall::Memset => {
                let dst = self.vm.read_arg::<u32>(0) as usize;
                let value = self.vm.read_arg::<u8>(1);
                let size = self.vm.read_arg::<u32>(2) as usize;
                self.vm.set_result(dst as u32);
                self.vm.data[dst..][..size].fill(value);
            }
            Syscall::Memcpy => {
                let dst = self.vm.read_arg::<u32>(0) as usize;
                let src = self.vm.read_arg::<u32>(1) as usize;
                let size = self.vm.read_arg::<u32>(2) as usize;
                self.vm.set_result(dst as u32);
                self.vm.data.copy_within(src..src + size, dst);
            }
            Syscall::Sin => {
                self.vm.set_result(cast(self.vm.read_arg::<f32>(0).sin()));
            }
            Syscall::Cos => {
                self.vm.set_result(cast(self.vm.read_arg::<f32>(0).cos()));
            }
            Syscall::Sqrt => {
                self.vm.set_result(cast(self.vm.read_arg::<f32>(0).sqrt()));
            }
            Syscall::Strncpy => {
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

fn main() {
    let mut game = Game::new(args().nth(1).unwrap());
    game.g_init(0, 0, false);
    game.g_run_frame(0);
    game.g_client_connect(0, true, false).unwrap();
    game.g_client_begin(0);
    let mut t = 8;
    for _ in 0..250 {
        let ps = game.vm.cast_mem_mut::<PlayerState>(game.clients);
        println!("{:?}", ps.origin);

        game.user_cmd.server_time = t;
        game.user_cmd.forward_move = 127;
        game.g_client_think(0);
        game.g_run_frame(t);
        t += 8;
    }
}
