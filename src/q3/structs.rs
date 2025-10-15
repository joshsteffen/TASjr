use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct CPlane {
    pub normal: [f32; 3],
    pub dist: f32,
    pub type_: u8,
    pub sign_bits: u8,
    pub pad: [u8; 2],
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Trace {
    pub all_solid: i32,
    pub start_solid: i32,
    pub fraction: f32,
    pub end_pos: [f32; 3],
    pub plane: CPlane,
    pub surface_flags: i32,
    pub contents: i32,
    pub entity_num: i32,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Trajectory {
    pub ty_type: i32,
    pub tr_time: i32,
    pub tr_duration: i32,
    pub tr_base: [f32; 3],
    pub tr_delta: [f32; 3],
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct EntityState {
    pub number: i32,
    pub e_type: i32,
    pub e_flags: i32,
    pub pos: Trajectory,
    pub apos: Trajectory,
    pub time: i32,
    pub time2: i32,
    pub origin: [f32; 3],
    pub origin2: [f32; 3],
    pub angles: [f32; 3],
    pub angles2: [f32; 3],
    pub other_entity_num: i32,
    pub other_entity_num2: i32,
    pub ground_entity_num: i32,
    pub constant_light: i32,
    pub loop_sound: i32,
    pub model_index: i32,
    pub model_index2: i32,
    pub client_num: i32,
    pub frame: i32,
    pub solid: i32,
    pub event: i32,
    pub event_parm: i32,
    pub powerups: i32,
    pub weapon: i32,
    pub legs_anim: i32,
    pub torso_anim: i32,
    pub generic1: i32,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct EntityShared {
    pub s: EntityState,
    pub linked: i32,
    pub link_count: i32,
    pub sv_flags: i32,
    pub single_client: i32,
    pub bmodel: i32,
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub contents: i32,
    pub absmin: [f32; 3],
    pub absmax: [f32; 3],
    pub current_origin: [f32; 3],
    pub current_angles: [f32; 3],
    pub owner_num: i32,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct SharedEntity {
    pub s: EntityState,
    pub r: EntityShared,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct PlayerState {
    pub command_time: i32,
    pub pm_type: i32,
    pub bob_cycle: i32,
    pub pm_flags: i32,
    pub pm_time: i32,
    pub origin: [f32; 3],
    pub velocity: [f32; 3],
    pub weapon_time: i32,
    pub gravity: i32,
    pub speed: i32,
    pub delta_angles: [i32; 3],
    pub ground_entity_num: i32,
    pub legs_timer: i32,
    pub legs_anim: i32,
    pub torso_timer: i32,
    pub torso_anim: i32,
    pub movement_dir: i32,
    pub grapple_point: [f32; 3],
    pub e_flags: i32,
    pub event_sequence: i32,
    pub events: [i32; 2],
    pub event_parms: [i32; 2],
    pub external_event: i32,
    pub external_event_parm: i32,
    pub external_event_time: i32,
    pub client_num: i32,
    pub weapon: i32,
    pub weapon_state: i32,
    pub view_angles: [f32; 3],
    pub view_height: i32,
    pub damage_event: i32,
    pub damage_yaw: i32,
    pub damage_pitch: i32,
    pub damage_count: i32,
    pub stats: [i32; 16],
    pub persistant: [i32; 16],
    pub powerups: [i32; 16],
    pub ammo: [i32; 16],
    pub generic1: i32,
    pub loop_sound: i32,
    pub jumppad_ent: i32,
    pub ping: i32,
    pub pmove_framecount: i32,
    pub jumppad_frame: i32,
    pub entity_event_sequence: i32,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct UserCmd {
    pub server_time: i32,
    pub angles: [i32; 3],
    pub buttons: i32,
    pub weapon: u8,
    pub forward_move: i8,
    pub right_move: i8,
    pub up_move: i8,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct VmCvar {
    pub handle: i32,
    pub modification_count: i32,
    pub value: f32,
    pub integer: i32,
    pub string: [u8; 256],
}
