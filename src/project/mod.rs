use std::path::PathBuf;

use crate::project::clip::Clip;

pub mod clip;
pub mod curve;

pub struct Project {
    pub bsp: PathBuf,
    pub clip: Clip,
}
