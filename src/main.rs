use std::{path::PathBuf, sync::Arc};

use bytemuck::pod_collect_to_vec;
use clap::Parser;
use eframe::egui;
use three_d::*;

use tasjr::{
    Snapshot,
    fs::Fs,
    game::Game,
    q3::{Map, playerState_t, usercmd_t},
    renderer::Renderer,
    ui::Timeline,
};

#[derive(clap::Parser)]
struct Args {
    /// Comma-separated list of root directories
    #[arg(short, long, value_delimiter = ',')]
    roots: Vec<PathBuf>,

    /// BSP to load
    #[arg()]
    bsp: PathBuf,

    /// User inputs to load
    #[arg()]
    usercmds: PathBuf,
}

struct App {
    game: Game,
    snapshots: Vec<<Game as Snapshot>::Snapshot>,
    usercmds: Vec<usercmd_t>,
    renderer: Arc<Renderer>,
    timeline: Timeline,
    playing: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        let args = Args::parse();
        let fs = Fs::new(&args.roots).unwrap();

        let mut buf = fs.read(&args.bsp).unwrap();
        Map::instance().load(args.bsp.to_str().unwrap(), &mut buf);

        let mut game = Game::new(&fs, "vm/qagame.qvm");
        game.cvars.set("dedicated", "1".to_string());
        game.cvars.set("df_promode", "1".to_string());
        game.init();
        game.vm.memory.clear_dirty();
        let baseline = game.take_snapshot(None);

        let mut deltas = vec![];
        let mut usercmds: Vec<usercmd_t> =
            pod_collect_to_vec(&std::fs::read(args.usercmds).unwrap());
        for usercmd in &mut usercmds {
            if game.relative_time() % 1000 == 0 {
                deltas.push(game.take_snapshot(Some(&baseline)));
            }
            let ps = game
                .vm
                .memory
                .cast_mut::<playerState_t>(game.clients.unwrap().address);
            (0..3).for_each(|i| usercmd.angles[i] -= ps.delta_angles[i]);
            game.run_frame(*usercmd);
        }
        let duration = usercmds.len() as f32 * 0.008;

        let mut renderer = Renderer::new(cc.gl.clone().unwrap());
        renderer.load_bsp(&fs, &args.bsp);

        Self {
            game,
            snapshots: deltas,
            usercmds,
            renderer: Arc::new(renderer),
            timeline: Timeline::new((0.0..=duration).into()),
            playing: false,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        let ps = self
            .game
            .vm
            .memory
            .cast_mut::<playerState_t>(self.game.clients.unwrap().address);
        let origin = Vec3::from(ps.origin) + vec3(0.0, 0.0, ps.viewheight as f32);
        let angles = ps.viewangles;

        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            self.playing = !self.playing;
        }

        if self.playing {
            self.timeline.playhead += ctx.input(|i| i.unstable_dt);
            if self.timeline.playhead >= self.timeline.max_range.max {
                self.timeline.playhead = self.timeline.max_range.max;
                self.playing = false;
            }
        }

        egui::TopBottomPanel::bottom("timeline")
            .resizable(true)
            .show(ctx, |ui| {
                ui.take_available_space();
                egui::Frame::NONE.show(ui, |ui| {
                    self.timeline.show(ui);
                });
            });

        egui::SidePanel::left("playerstate")
            .resizable(true)
            .show(ctx, |ui| {
                ui.take_available_space();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.take_available_space();
                    ui.label(format!("{:#?}", ps));
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Frame::NONE.show(ui, |ui| {
                ui.take_available_space();
                let renderer = Arc::clone(&self.renderer);
                ui.painter().add(egui::PaintCallback {
                    rect: ui.min_rect(),
                    callback: Arc::new(eframe::egui_glow::CallbackFn::new(
                        move |info, _painter| {
                            renderer.render(info, origin, angles.into());
                        },
                    )),
                })
            });
        });

        let ms = (self.timeline.playhead * 1000.0) as i32;
        let ms = ms - ms % 8;
        if self.game.relative_time() > ms || self.game.relative_time() / 1000 != ms / 1000 {
            self.game
                .restore_from_snapshot(&self.snapshots[ms as usize / 1000]);
        }
        while self.game.relative_time() < ms {
            self.game
                .run_frame(self.usercmds[self.game.relative_time() as usize / 8]);
        }
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "TASjr",
        eframe::NativeOptions {
            depth_buffer: 24,
            multisampling: 8,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
