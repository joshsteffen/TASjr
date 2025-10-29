#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::{
    ffi::{CStr, CString},
    sync::{LazyLock, Mutex, MutexGuard},
};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub fn angle_to_short(x: f32) -> u16 {
    (x * u16::MAX as f32 / 360.0) as i32 as u16
}

pub fn short_to_angle(x: u16) -> f32 {
    x as f32 * 360.0 / u16::MAX as f32
}

/// A safe wrapper around functions related to the currently loaded map.
pub struct Map {
    pub entity_tokens: std::vec::IntoIter<String>,
    loaded: bool,
}

impl Map {
    pub fn instance<'a>() -> MutexGuard<'a, Self> {
        static MAP: LazyLock<Mutex<Map>> = LazyLock::new(|| {
            unsafe { Com_Init() };
            Mutex::new(Map {
                loaded: false,
                entity_tokens: vec![].into_iter(),
            })
        });

        MAP.lock().unwrap()
    }

    pub fn load(&mut self, name: &str, buf: &mut [u8]) {
        if self.loaded {
            todo!();
        }
        unsafe {
            CM_LoadMap(
                CString::new(name).unwrap().as_ptr(),
                buf.as_mut_ptr().cast(),
                buf.len().try_into().unwrap(),
            );

            let mut p = CM_EntityString().cast_const();
            assert!(!p.is_null());

            let mut entity_tokens = vec![];
            loop {
                let s = COM_Parse(&mut p);
                if s.is_null() || *s == 0 {
                    break;
                }
                entity_tokens.push(CStr::from_ptr(s).to_str().unwrap().to_string());
            }
            self.entity_tokens = entity_tokens.into_iter();
            self.loaded = true;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn box_trace(
        &self,
        trace: &mut trace_t,
        start: &vec3_t,
        end: &vec3_t,
        mins: &vec3_t,
        maxs: &vec3_t,
        model: clipHandle_t,
        brushmask: i32,
        capsule: bool,
    ) {
        assert!(self.loaded);
        unsafe {
            CM_BoxTrace(
                trace,
                start.as_ptr(),
                end.as_ptr(),
                mins.as_ptr(),
                maxs.as_ptr(),
                model,
                brushmask,
                capsule as qboolean,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn transformed_box_trace(
        &self,
        trace: &mut trace_t,
        start: &vec3_t,
        end: &vec3_t,
        mins: &vec3_t,
        maxs: &vec3_t,
        model: clipHandle_t,
        brushmask: i32,
        origin: &vec3_t,
        angles: &vec3_t,
        capsule: bool,
    ) {
        assert!(self.loaded);
        unsafe {
            CM_TransformedBoxTrace(
                trace,
                start.as_ptr(),
                end.as_ptr(),
                mins.as_ptr(),
                maxs.as_ptr(),
                model,
                brushmask,
                origin.as_ptr(),
                angles.as_ptr(),
                capsule as qboolean,
            );
        }
    }

    pub fn point_contents(&self, p: &vec3_t, model: clipHandle_t) -> i32 {
        assert!(self.loaded);
        unsafe { CM_PointContents(p.as_ptr(), model) }
    }

    pub fn inline_model(&self, index: i32) -> clipHandle_t {
        assert!(self.loaded);
        unsafe { CM_InlineModel(index) }
    }

    pub fn model_bounds(&self, model: clipHandle_t, mins: &mut vec3_t, maxs: &mut vec3_t) {
        assert!(self.loaded);
        unsafe { CM_ModelBounds(model, mins.as_mut_ptr(), maxs.as_mut_ptr()) }
    }

    pub fn temp_box_model(&self, mins: &vec3_t, maxs: &vec3_t, capsule: bool) -> clipHandle_t {
        assert!(self.loaded);
        unsafe { CM_TempBoxModel(mins.as_ptr(), maxs.as_ptr(), capsule as i32) }
    }
}
