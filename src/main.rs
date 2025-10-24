use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use bytemuck::pod_collect_to_vec;
use clap::Parser;
use eframe::egui;

use tasjr::{
    fs::Fs,
    q3::{Map, usercmd_t},
    renderer::Renderer,
    run::Run,
    ui::{
        Timeline,
        viewport::{FlyCam, first_person_ui},
    },
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
    run: Run,
    renderer: Arc<Mutex<Renderer>>,
    timeline: Timeline,
    playing: bool,
    flycam: FlyCam,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        let args = Args::parse();
        let fs = Fs::new(&args.roots).unwrap();

        let mut buf = fs.read(&args.bsp).unwrap();
        Map::instance().load(args.bsp.to_str().unwrap(), &mut buf);

        let mut run = Run::new(&fs);

        let usercmds: Vec<usercmd_t> = pod_collect_to_vec(&std::fs::read(args.usercmds).unwrap());
        let duration = (usercmds.len() - 1) as f32 * 0.008;
        run.set_usercmds(0, &usercmds);

        let mut renderer = Renderer::new(cc.gl.clone().unwrap());
        renderer.load_bsp(&fs, &args.bsp);

        Self {
            run,
            renderer: Arc::new(Mutex::new(renderer)),
            timeline: Timeline::new((0.0..=duration).into()),
            playing: false,
            flycam: Default::default(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        let frame = (self.timeline.playhead * 1000.0) as usize / 8;

        self.renderer
            .lock()
            .unwrap()
            .set_player_origin(self.run.game.ps().origin.into());

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
                self.timeline.show(ui, &self.run);
            });

        egui::SidePanel::left("playerstate")
            .resizable(true)
            .show(ctx, |ui| {
                ui.take_available_space();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.take_available_space();
                    ui.label(format!("{:#?}", self.run.game.ps()));
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.columns_const(|[left_ui, right_ui]| {
                self.flycam.ui(left_ui, Arc::clone(&self.renderer));
                first_person_ui(right_ui, Arc::clone(&self.renderer), &mut self.run, frame);
            });
        });

        self.run.seek(frame);
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
