pub mod bsp;
pub mod fs;
pub mod game;
pub mod q3;
pub mod renderer;
pub mod vm;

pub trait Snapshot {
    type Snapshot;

    fn take_snapshot(&self) -> Self::Snapshot;
    fn restore_from_snapshot(&mut self, snapshot: &Self::Snapshot);
}
