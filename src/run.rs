use std::{
    sync::{Arc, Mutex},
    thread,
};

use bytemuck::Zeroable;

use crate::{Snapshot as _, fs::Fs, game::Game, q3::usercmd_t};

pub const SNAPSHOT_INTERVAL: usize = 125;

type Snapshot = <Game as crate::Snapshot>::Snapshot;

/// Data that is shared between threads and should all be locked at once.
struct Shared {
    /// User inputs for each frame of simulation.
    usercmds: Vec<usercmd_t>,

    /// Cached state of the whole game every `SNAPSHOT_INTERVAL` frames to speed up seeking.
    snapshots: Vec<Option<Snapshot>>,

    /// The number of up-to-date snapshots.
    num_valid_snapshots: usize,

    /// The number of usercmds the snapshot thread is aware of.
    num_processed_usercmds: usize,
}

impl Shared {
    fn has_valid_snapshot(&self, frame: usize) -> bool {
        frame < self.num_valid_snapshots * SNAPSHOT_INTERVAL
    }

    fn invalidate(&mut self, frame: usize) {
        self.num_valid_snapshots = self.num_valid_snapshots.min(frame / SNAPSHOT_INTERVAL + 1);
        self.num_processed_usercmds = self.num_processed_usercmds.min(frame);
    }
}

pub struct Run {
    pub game: Game,
    shared: Arc<Mutex<Shared>>,

    /// A snapshot taken after initialization but before any user input occurs that all other
    /// snapshots are based on.
    baseline: Arc<Snapshot>,

    /// Generates snapshots in the background when usercmds change.
    snapshot_worker: thread::JoinHandle<()>,
    snapshot_worker_enabled: bool,

    /// Is the current state of `game` based on old usercmds?
    stale: bool,
}

impl Run {
    pub fn new(fs: &Fs) -> Self {
        let mut game = Game::new(fs, "vm/qagame.qvm");
        game.cvars.set("dedicated", "1".to_string());
        game.cvars.set("df_promode", "1".to_string());
        game.init();
        game.vm.memory.clear_dirty();
        let baseline = Arc::new(game.take_snapshot(None));

        let shared = Arc::new(Mutex::new(Shared {
            usercmds: vec![],
            snapshots: vec![Some(game.take_snapshot(Some(&baseline)))],
            num_valid_snapshots: 1,
            num_processed_usercmds: 0,
        }));

        let snapshot_thread = {
            // Make a copy of the game just for generating snapshots so we don't need to lock it
            let mut game = game.clone();

            let shared = Arc::clone(&shared);
            let baseline = Arc::clone(&baseline);

            thread::spawn(move || {
                loop {
                    // Wait for invalid snapshots to work on
                    loop {
                        {
                            let shared = shared.lock().unwrap();
                            if shared.num_valid_snapshots < shared.snapshots.len() {
                                break;
                            }
                        }
                        thread::park();
                    }

                    // Start from the last valid snapshot
                    let next_snapshot_num;
                    let num_processed_usercmds;
                    let usercmds;
                    {
                        let mut shared = shared.lock().unwrap();

                        shared.num_processed_usercmds =
                            (shared.num_processed_usercmds + 1).next_multiple_of(SNAPSHOT_INTERVAL);
                        num_processed_usercmds = shared.num_processed_usercmds;

                        next_snapshot_num = num_processed_usercmds / SNAPSHOT_INTERVAL;

                        usercmds = shared.usercmds[(next_snapshot_num - 1) * SNAPSHOT_INTERVAL..]
                            [..SNAPSHOT_INTERVAL]
                            .to_owned();

                        // TODO: Could this hold the lock for too long? If so maybe we could box
                        // the snaphots and temporarily .take() the one we neeed and decrement
                        // num_valid_snapshots while restoring. Boxing would make moving cheap,
                        // which might also speed up adding the new valid snapshot below.
                        game.restore_from_snapshot(
                            shared.snapshots[next_snapshot_num - 1].as_ref().unwrap(),
                        );
                    }

                    // Simulate to the next snapshot
                    for usercmd in usercmds {
                        game.run_frame(usercmd);
                    }
                    let snapshot = game.take_snapshot(Some(&baseline));

                    // Save it unless it's already been invalidated again
                    {
                        let mut shared = shared.lock().unwrap();
                        if shared.num_processed_usercmds == num_processed_usercmds {
                            shared.snapshots[next_snapshot_num] = Some(snapshot);
                            shared.num_valid_snapshots = next_snapshot_num + 1;
                        }
                    }
                }
            })
        };

        Self {
            game,
            shared,
            baseline,
            snapshot_worker: snapshot_thread,
            snapshot_worker_enabled: true,
            stale: false,
        }
    }

    pub fn set_usercmds(&mut self, start_frame: usize, usercmds: &[usercmd_t]) {
        if start_frame < self.game.frame() {
            self.stale = true;
        }

        let mut shared = self.shared.lock().unwrap();

        let new_len = shared.usercmds.len().max(start_frame + usercmds.len());
        shared.usercmds.resize(new_len, usercmd_t::zeroed());
        shared.usercmds[start_frame..][..usercmds.len()].copy_from_slice(usercmds);

        let new_num_snapshots = new_len / SNAPSHOT_INTERVAL + 1;
        shared.snapshots.resize_with(new_num_snapshots, || None);

        shared.invalidate(start_frame);
        if self.snapshot_worker_enabled {
            self.snapshot_worker.thread().unpark();
        }
    }

    pub fn with_usercmd_mut<R>(&mut self, frame: usize, f: impl FnOnce(&mut usercmd_t) -> R) -> R {
        if frame < self.game.frame() {
            self.stale = true;
        }

        let mut shared = self.shared.lock().unwrap();

        let usercmd = &mut shared.usercmds[frame];
        let result = f(usercmd);

        shared.invalidate(frame);
        if self.snapshot_worker_enabled {
            self.snapshot_worker.thread().unpark();
        }

        result
    }

    pub fn with_usercmd<R>(&mut self, frame: usize, f: impl FnOnce(&usercmd_t) -> R) -> R {
        let shared = self.shared.lock().unwrap();
        let usercmd = &shared.usercmds[frame];
        f(usercmd)
    }

    pub fn seek(&mut self, frame: usize) {
        if !self.stale && self.game.frame() == frame + 1 {
            // If we're just going to run the previous frame again but nothing has changed, we'll
            // just end up exactly where we are now.
            return;
        }

        let mut shared = self.shared.lock().unwrap();

        if !self.can_step_to(frame) {
            if !shared.has_valid_snapshot(frame) {
                self.stale = true;
                return;
            }
            let snapshot = shared.snapshots[frame / SNAPSHOT_INTERVAL]
                .as_ref()
                .unwrap();
            self.game.restore_from_snapshot(snapshot);
            self.stale = false;
        }

        while self.game.frame() <= frame {
            self.game.run_frame(shared.usercmds[self.game.frame()]);

            if !self.snapshot_worker_enabled && self.game.frame() % SNAPSHOT_INTERVAL == 0 {
                let snapshot_num = self.game.frame() / SNAPSHOT_INTERVAL;
                assert!(shared.num_valid_snapshots >= snapshot_num);
                if shared.num_valid_snapshots == snapshot_num {
                    shared.snapshots[snapshot_num] =
                        Some(self.game.take_snapshot(Some(&self.baseline)));
                    shared.num_valid_snapshots = snapshot_num + 1;
                }
            }
        }
    }

    fn can_step_to(&self, frame: usize) -> bool {
        // If the current state is valid and we're not trying to rewind or seek too far ahead then
        // we can just simulate forward.
        let forward_seekable_range = self.game.frame()..=self.game.frame() + SNAPSHOT_INTERVAL;
        !self.stale && forward_seekable_range.contains(&frame)
    }

    pub fn can_seek_to(&self, frame: usize) -> bool {
        let shared = self.shared.lock().unwrap();
        self.can_step_to(frame) || shared.has_valid_snapshot(frame)
    }

    pub fn num_frames_with_valid_snapshot(&self) -> usize {
        self.shared.lock().unwrap().num_valid_snapshots * SNAPSHOT_INTERVAL
    }

    pub fn enable_snapshot_worker(&mut self) {
        if !self.snapshot_worker_enabled {
            self.snapshot_worker_enabled = true;
            self.snapshot_worker.thread().unpark();
        }
    }

    pub fn disable_snapshot_worker(&mut self) {
        self.snapshot_worker_enabled = false;
    }
}
