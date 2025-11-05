use std::collections::BTreeMap;

use bytemuck::Zeroable;

use crate::{
    project::curve::{Curve, Interpolation, Keyframe},
    q3::usercmd_t,
};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum Parameter {
    Pitch,
    Yaw,
    Roll,
    Button(u8),
    Weapon,
    ForwardMove,
    RightMove,
    UpMove,
}

impl Parameter {
    pub fn range(&self) -> (isize, isize) {
        match self {
            Parameter::Pitch | Parameter::Yaw | Parameter::Roll => (0, 65535),
            Parameter::Button(_) => (0, 1),
            Parameter::Weapon => (0, 15),
            Parameter::ForwardMove | Parameter::RightMove | Parameter::UpMove => (-127, 127),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Parameter::Pitch => "Pitch".to_string(),
            Parameter::Yaw => "Yaw".to_string(),
            Parameter::Roll => "Roll".to_string(),
            Parameter::Button(0) => "Attack".to_string(),
            Parameter::Button(i) => format!("Button {}", i + 1),
            Parameter::Weapon => "Weapon".to_string(),
            Parameter::ForwardMove => "Forward".to_string(),
            Parameter::RightMove => "Right".to_string(),
            Parameter::UpMove => "Up".to_string(),
        }
    }
}

pub struct Clip {
    pub curves: BTreeMap<Parameter, Curve>,
    pub len: usize,
}

impl Clip {
    pub fn new() -> Self {
        let mut curves = BTreeMap::new();
        curves.insert(Parameter::Pitch, Curve::default());
        curves.insert(Parameter::Yaw, Curve::default());
        curves.insert(Parameter::Button(0), Curve::default());
        curves.insert(Parameter::Weapon, Curve::default());
        curves.insert(Parameter::ForwardMove, Curve::default());
        curves.insert(Parameter::RightMove, Curve::default());
        curves.insert(Parameter::UpMove, Curve::default());
        Self { curves, len: 0 }
    }

    pub fn dirty(&self) -> usize {
        let mut dirty = usize::MAX;
        for curve in self.curves.values() {
            dirty = dirty.min(curve.dirty());
        }
        dirty
    }

    pub fn clear_dirty(&mut self) {
        for curve in self.curves.values_mut() {
            curve.clear_dirty();
        }
    }

    pub fn remove_keyframe(&mut self, frame: usize) {
        for curve in self.curves.values_mut() {
            curve.remove_keyframe(frame);
        }
    }

    pub fn usercmd(&self, frame: usize) -> usercmd_t {
        let mut usercmd = usercmd_t::zeroed();
        for (parameter, curve) in &self.curves {
            let value = curve.eval(frame);
            match parameter {
                Parameter::Pitch => usercmd.angles[0] = value as i32,
                Parameter::Yaw => usercmd.angles[1] = value as i32,
                Parameter::Roll => usercmd.angles[2] = value as i32,
                Parameter::Button(i) => usercmd.buttons |= ((value != 0) as i32) << i,
                Parameter::Weapon => usercmd.weapon = value as u8,
                Parameter::ForwardMove => usercmd.forwardmove = value as i8,
                Parameter::RightMove => usercmd.rightmove = value as i8,
                Parameter::UpMove => usercmd.upmove = value as i8,
            }
        }
        usercmd
    }

    pub fn set_usercmd(&mut self, frame: usize, usercmd: usercmd_t) {
        self.len = self.len.max(frame + 1);

        let interp = Interpolation::Hold;

        // TODO: Set all params, even ones that aren't in the map yet?
        for (parameter, curve) in &mut self.curves {
            let value = match parameter {
                Parameter::Pitch => usercmd.angles[0] as isize,
                Parameter::Yaw => usercmd.angles[1] as isize,
                Parameter::Roll => usercmd.angles[2] as isize,
                Parameter::Button(i) => (usercmd.buttons & (1 << i)) as isize,
                Parameter::Weapon => usercmd.weapon as isize,
                Parameter::ForwardMove => usercmd.forwardmove as isize,
                Parameter::RightMove => usercmd.rightmove as isize,
                Parameter::UpMove => usercmd.upmove as isize,
            };

            // TODO: Only add keyframe if it would change the curve's value at that point?
            curve.insert_keyframe(Keyframe::new(frame, value, interp));
        }
    }

    pub fn optimize(&mut self) {
        for curve in self.curves.values_mut() {
            curve.optimize();
        }
    }
}

impl Default for Clip {
    fn default() -> Self {
        Self::new()
    }
}
