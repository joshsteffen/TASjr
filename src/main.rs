use std::{path::PathBuf, sync::Arc};

use bytemuck::Zeroable;
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
}

struct App {
    game: Game,
    snapshots: Vec<<Game as Snapshot>::Snapshot>,
    usercmds: Vec<usercmd_t>,
    renderer: Arc<Renderer>,
    timeline: Timeline,
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

        // Record some dummy data
        let mut deltas = vec![];
        let mut usercmds = vec![];
        deltas.push(game.take_snapshot(Some(&baseline)));
        while game.time < 30000 {
            let usercmd = usercmd_t {
                forwardmove: 127,
                rightmove: if game.time % 3000 < 1500 { 127 } else { -127 },
                ..Zeroable::zeroed()
            };
            usercmds.push(usercmd);
            game.run_frame(usercmd);
            if game.time % 1000 == 0 {
                deltas.push(game.take_snapshot(Some(&baseline)));
            }
        }

        let mut renderer = Renderer::new(cc.gl.clone().unwrap());
        renderer.load_bsp(&fs, &args.bsp);

        Self {
            game,
            snapshots: deltas,
            usercmds,
            renderer: Arc::new(renderer),
            timeline: Timeline::new((0.0..=30.0).into()),
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

        egui::TopBottomPanel::bottom("timeline")
            .resizable(true)
            .show(ctx, |ui| {
                ui.take_available_space();
                egui::Frame::NONE.show(ui, |ui| {
                    self.timeline.show(ui);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns(2, |columns| {
                egui::Frame::NONE.show(&mut columns[0], |ui| {
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
                egui::Frame::NONE.show(&mut columns[1], |ui| {
                    ui.take_available_space();
                    ui.label("test");
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
        });

        let ms = (self.timeline.playhead * 1000.0) as i32;
        let ms = ms - ms % 8;
        if self.game.time > ms || self.game.time / 1000 != ms / 1000 {
            self.game
                .restore_from_snapshot(&self.snapshots[ms as usize / 1000]);
        }
        while self.game.time < ms {
            self.game
                .run_frame(self.usercmds[self.game.time as usize / 8]);
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
