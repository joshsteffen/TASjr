#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::{
    ffi::{CStr, CString},
    sync::{LazyLock, Mutex, MutexGuard},
};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

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

    pub fn point_contents(&self, p: &vec3_t, model: clipHandle_t) -> i32 {
        assert!(self.loaded);
        unsafe { CM_PointContents(p.as_ptr(), model) }
    }
}
