pub mod animation;
pub mod bsp;
pub mod fs;
pub mod game;
pub mod q3;
pub mod renderer;
pub mod run;
pub mod ui;
pub mod vm;

pub trait Snapshot {
    type Snapshot;

    fn take_snapshot(&self, baseline: Option<&Self::Snapshot>) -> Self::Snapshot;
    fn restore_from_snapshot(&mut self, snapshot: &Self::Snapshot);
}
