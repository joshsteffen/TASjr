use std::{
    sync::{Arc, Mutex},
    thread,
};

use bytemuck::Zeroable;

use crate::{
    Snapshot as _,
    animation::{Curve, Interpolation, Keyframe},
    fs::Fs,
    game::Game,
    q3::usercmd_t,
};

pub const SNAPSHOT_INTERVAL: usize = 125;

type Snapshot = <Game as crate::Snapshot>::Snapshot;

pub enum InputKind {
    Angle(u8),
    Button(u8),
    Weapon,
    Move(u8),
}

pub struct Input {
    pub name: String,
    pub kind: InputKind,
    pub curve: Curve,
}

impl Input {
    fn new(name: impl ToString, kind: InputKind) -> Self {
        Self {
            name: name.to_string(),
            kind,
            curve: Default::default(),
        }
    }

    pub fn range(&self) -> (isize, isize) {
        match self.kind {
            InputKind::Angle(_) => (0, 65535),
            InputKind::Button(_) => (0, 1),
            InputKind::Weapon => (0, 15),
            InputKind::Move(_) => (-128, 127),
        }
    }
}

pub struct Inputs {
    pub angles: [Input; 3],
    pub buttons: [Input; 1], // TODO
    pub weapon: Input,
    pub forwardmove: Input,
    pub rightmove: Input,
    pub upmove: Input,
    pub len: usize,
}

impl Inputs {
    fn new() -> Self {
        Self {
            angles: [
                Input::new("Pitch", InputKind::Angle(0)),
                Input::new("Yaw", InputKind::Angle(1)),
                Input::new("Roll", InputKind::Angle(2)),
            ],
            buttons: [Input::new("Attack", InputKind::Button(0))],
            weapon: Input::new("Weapon", InputKind::Weapon),
            forwardmove: Input::new("Forward", InputKind::Move(0)),
            rightmove: Input::new("Back", InputKind::Move(1)),
            upmove: Input::new("Up", InputKind::Move(2)),
            len: 0,
        }
    }

    pub fn all(&self) -> impl Iterator<Item = &Input> {
        self.angles
            .iter()
            .chain(self.buttons.iter())
            .chain(std::iter::once(&self.weapon))
            .chain(std::iter::once(&self.forwardmove))
            .chain(std::iter::once(&self.rightmove))
            .chain(std::iter::once(&self.upmove))
    }

    pub fn all_mut(&mut self) -> impl Iterator<Item = &mut Input> {
        self.angles
            .iter_mut()
            .chain(self.buttons.iter_mut())
            .chain(std::iter::once(&mut self.weapon))
            .chain(std::iter::once(&mut self.forwardmove))
            .chain(std::iter::once(&mut self.rightmove))
            .chain(std::iter::once(&mut self.upmove))
    }

    fn dirty(&self) -> usize {
        let mut dirty = usize::MAX;
        for input in self.all() {
            dirty = dirty.min(input.curve.dirty());
        }
        dirty
    }

    fn clear_dirty(&mut self) {
        for input in self.all_mut() {
            input.curve.clear_dirty();
        }
    }

    pub fn remove_keyframe(&mut self, frame: usize) {
        for input in self.all_mut() {
            input.curve.remove_keyframe(frame);
        }
    }

    pub fn usercmd(&self, frame: usize) -> usercmd_t {
        let mut usercmd = usercmd_t::zeroed();
        for input in self.all() {
            let value = input.curve.eval(frame);
            match input.kind {
                InputKind::Angle(i) => usercmd.angles[i as usize] = value as i32,
                InputKind::Button(i) => usercmd.buttons |= ((value != 0) as i32) << i,
                InputKind::Weapon => usercmd.weapon = value as u8,
                InputKind::Move(i) => {
                    *[
                        &mut usercmd.forwardmove,
                        &mut usercmd.rightmove,
                        &mut usercmd.upmove,
                    ][i as usize] = value as i8
                }
            }
        }
        usercmd
    }

    pub fn set_usercmd(&mut self, frame: usize, usercmd: usercmd_t) {
        self.len = self.len.max(frame + 1);

        let interp = Interpolation::Hold;

        for input in self.all_mut() {
            let value = match input.kind {
                InputKind::Angle(i) => usercmd.angles[i as usize] as isize,
                InputKind::Button(i) => ((usercmd.buttons & (1 << i)) != 0) as isize,
                InputKind::Weapon => usercmd.weapon as isize,
                InputKind::Move(i) => {
                    [usercmd.forwardmove, usercmd.rightmove, usercmd.upmove][i as usize] as isize
                }
            };

            input
                .curve
                .insert_keyframe(Keyframe::new(frame, value, interp));
        }
    }

    pub fn optimize(&mut self) {
        for input in self.all_mut() {
            input.curve.optimize();
        }
    }
}

/// Data that is shared between threads and should all be locked at once.
struct Shared {
    /// User inputs for each frame of simulation.
    inputs: Inputs,

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
            inputs: Inputs::new(),
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
                        thread::park();
                        {
                            let shared = shared.lock().unwrap();
                            if shared.num_valid_snapshots < shared.snapshots.len() {
                                break;
                            }
                        }
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

                        let start = (next_snapshot_num - 1) * SNAPSHOT_INTERVAL;
                        let end = start + SNAPSHOT_INTERVAL;
                        usercmds = (start..end)
                            .map(|frame| shared.inputs.usercmd(frame))
                            .collect::<Vec<_>>();

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

    pub fn with_inputs<R>(&mut self, f: impl FnOnce(&Inputs) -> R) -> R {
        f(&self.shared.lock().unwrap().inputs)
    }

    pub fn with_inputs_mut<R>(&mut self, f: impl FnOnce(&mut Inputs) -> R) -> R {
        let mut shared = self.shared.lock().unwrap();

        shared.inputs.clear_dirty();
        let result = f(&mut shared.inputs);
        let dirty = shared.inputs.dirty();

        shared.invalidate(dirty);
        if dirty < self.game.frame() {
            self.stale = true;
        }

        let num_snapshots = shared.inputs.len / SNAPSHOT_INTERVAL + 1;
        if num_snapshots > shared.snapshots.len() {
            shared.snapshots.resize_with(num_snapshots, || None);
        }

        if self.snapshot_worker_enabled {
            self.snapshot_worker.thread().unpark();
        }

        result
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
            self.game
                .run_frame(shared.inputs.usercmd(self.game.frame()));

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
