use std::{
    ffi::CStr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use bytemuck::Zeroable;
use clap::Parser;
use eframe::egui;
use three_d::*;

use tasjr::{
    Snapshot,
    fs::Fs,
    game::{Game, GameSnapshot},
    q3::{CM_EntityString, CM_LoadMap, COM_Parse, Com_Init, playerState_t, usercmd_t},
    renderer::Renderer,
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
    snapshot: GameSnapshot,
    usercmd: usercmd_t,
    renderer: Arc<Mutex<Renderer>>,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        let args = Args::parse();
        let fs = Fs::new(&args.roots).unwrap();

        let mut buf = fs.read(&args.bsp).unwrap();
        let mut entity_tokens = vec![];
        unsafe {
            Com_Init();
            CM_LoadMap(c"q3dm6".as_ptr(), buf.as_mut_ptr().cast(), buf.len() as i32);
            let mut p = CM_EntityString().cast_const();
            loop {
                let s = COM_Parse(&mut p);
                if s.is_null() || *s == 0 {
                    break;
                }
                entity_tokens.push(CStr::from_ptr(s).to_str().unwrap().to_string());
            }
        }

        let mut game = Game::new(&fs, "vm/qagame.qvm", entity_tokens);
        game.cvars.set("dedicated", "1".to_string());
        game.cvars.set("df_promode", "1".to_string());
        game.init();
        game.vm.memory.dirty.clear();
        let snapshot = game.take_snapshot();

        let mut renderer = Renderer::new(cc.gl.clone().unwrap());
        renderer.load_bsp(&fs, &args.bsp);

        Self {
            game,
            snapshot,
            usercmd: usercmd_t::zeroed(),
            renderer: Arc::new(Mutex::new(renderer)),
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

        let renderer = Arc::clone(&self.renderer);

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Frame::canvas(ui.style()).show(ui, |ui| {
                ui.take_available_space();

                let response = ui.interact(ui.min_rect(), ui.id().with("3d"), egui::Sense::drag());
                if response.dragged() {
                    let delta = response.drag_delta();
                    self.usercmd.angles[1] -= (delta.x * 100.0) as i32;
                    self.usercmd.angles[0] += (delta.y * 100.0) as i32;
                }

                ui.painter().add(egui::PaintCallback {
                    rect: ui.min_rect(),
                    callback: Arc::new(eframe::egui_glow::CallbackFn::new(
                        move |info, _painter| {
                            renderer.lock().unwrap().render(info, origin, angles.into());
                        },
                    )),
                })
            });
        });

        ctx.input(|i| {
            use egui::Key::*;

            self.usercmd.forwardmove = 127 * (i.key_down(W) as i8 - i.key_down(S) as i8);
            self.usercmd.rightmove = 127 * (i.key_down(D) as i8 - i.key_down(A) as i8);
            self.usercmd.upmove = 127 * (i.key_down(Space) as i8 - i.key_down(C) as i8);

            if i.key_pressed(Backspace) {
                self.game.restore_from_snapshot(&self.snapshot);
            }
            if i.key_pressed(Enter) {
                self.snapshot = self.game.take_snapshot();
                self.game.vm.memory.clear_dirty();
            }
        });

        self.game.run_frame(self.usercmd);
        self.game.run_frame(self.usercmd);
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "TASjr",
        eframe::NativeOptions {
            depth_buffer: 24,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
