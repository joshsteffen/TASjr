use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use bytemuck::pod_collect_to_vec;
use clap::Parser;
use eframe::{egui, glow};

use crate::{
    fs::Fs,
    q3::{Map, usercmd_t},
    renderer::Renderer,
    run::Run,
    ui::{
        Timeline,
        theme::set_theme,
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

struct AppState {
    run: Run,
    renderer: Arc<Mutex<Renderer>>,
    timeline: Timeline,
    flycam: FlyCam,
}

impl AppState {
    fn new(gl: Arc<glow::Context>) -> Self {
        let args = Args::parse();
        let fs = Fs::new(&args.roots).unwrap();

        let mut buf = fs.read(&args.bsp).unwrap();
        Map::instance().load(args.bsp.to_str().unwrap(), &mut buf);

        let mut run = Run::new(&fs);

        let usercmds: Vec<usercmd_t> = pod_collect_to_vec(&std::fs::read(args.usercmds).unwrap());
        let duration = (usercmds.len() - 1) as f32 * 0.008;
        run.set_usercmds(0, &usercmds);

        let mut renderer = Renderer::new(gl);
        renderer.load_bsp(&fs, &args.bsp);

        Self {
            run,
            renderer: Arc::new(Mutex::new(renderer)),
            timeline: Timeline::new((0.0..=duration).into()),
            flycam: Default::default(),
        }
    }
}

enum Tab {
    FirstPerson,
    FlyCam,
    PlayerState,
    Timeline,
}

impl egui_dock::TabViewer for AppState {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        match tab {
            Tab::FirstPerson => "First-person view",
            Tab::FlyCam => "Fly camera",
            Tab::PlayerState => "Player state inspector",
            Tab::Timeline => "Timeline",
        }
        .into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match tab {
            Tab::FirstPerson => {
                first_person_ui(
                    ui,
                    Arc::clone(&self.renderer),
                    &mut self.timeline,
                    &mut self.run,
                );
            }
            Tab::FlyCam => {
                self.flycam.ui(ui, Arc::clone(&self.renderer));
            }
            Tab::PlayerState => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.take_available_space();
                    ui.label(format!("{:#?}", self.run.game.ps()));
                });
            }
            Tab::Timeline => {
                self.timeline.show(ui, &self.run);
            }
        }
    }
}

pub struct App {
    app_state: AppState,
    dock_state: egui_dock::DockState<Tab>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        set_theme(&cc.egui_ctx);

        let app_state = AppState::new(cc.gl.clone().unwrap());

        let mut dock_state = egui_dock::DockState::new(vec![Tab::Timeline]);

        let [_, ps] = dock_state.main_surface_mut().split_above(
            egui_dock::NodeIndex::root(),
            0.5,
            vec![Tab::PlayerState],
        );

        let [_, fly] = dock_state
            .main_surface_mut()
            .split_right(ps, 0.125, vec![Tab::FlyCam]);

        let [_, _] = dock_state
            .main_surface_mut()
            .split_right(fly, 0.5, vec![Tab::FirstPerson]);

        Self {
            app_state,
            dock_state,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        self.app_state
            .renderer
            .lock()
            .unwrap()
            .update(&self.app_state.run);

        // Space is the standard play/pause key, but it's also jump, so enter also works even
        // during recording.
        if ctx.input(|i| {
            !self.app_state.timeline.recording && i.key_pressed(egui::Key::Space)
                || i.key_pressed(egui::Key::Enter)
        }) {
            self.app_state.timeline.playing = !self.app_state.timeline.playing;
        }

        self.app_state.timeline.update(ctx.input(|i| i.unstable_dt));

        if self.app_state.timeline.recording {
            self.app_state.run.disable_snapshot_worker();
        } else {
            self.app_state.run.enable_snapshot_worker();
        }

        egui_dock::DockArea::new(&mut self.dock_state)
            .show_close_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show_leaf_collapse_buttons(false)
            .show(ctx, &mut self.app_state);

        self.app_state.run.seek(self.app_state.timeline.frame());
    }
}
