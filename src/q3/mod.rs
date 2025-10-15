pub use structs::*;

mod structs;

#[link(name = "q3")]
unsafe extern "C" {
    pub fn Com_Init();
    pub fn COM_Parse(data_p: *mut *const i8) -> *const i8;
    pub fn CM_LoadMap(name: *const i8, buf: *const u8, length: i32);
    pub fn CM_EntityString() -> *const i8;
    pub fn CM_BoxTrace(
        results: *mut Trace,
        start: *const [f32; 3],
        end: *const [f32; 3],
        mins: *const [f32; 3],
        maxs: *const [f32; 3],
        model: i32,
        brush_mask: i32,
        capsule: i32,
    );

}
