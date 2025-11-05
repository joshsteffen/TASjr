use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use bytemuck::pod_collect_to_vec;
use clap::Parser;
use eframe::{egui, glow};

use crate::{
    fs::Fs,
    project::Project,
    q3::{Map, usercmd_t},
    renderer::Renderer,
    session::Session,
    ui::{
        Timeline,
        clip_editor::clip_editor_ui,
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
    session: Session,
    renderer: Arc<Mutex<Renderer>>,
    timeline: Timeline,
    flycam: FlyCam,
}

impl AppState {
    fn new(gl: Arc<glow::Context>) -> Self {
        let args = Args::parse();
        let fs = Fs::new(&args.roots).unwrap();

        let mut project = Project {
            bsp: args.bsp.to_path_buf(),
            clip: Default::default(),
        };

        let usercmds: Vec<usercmd_t> = pod_collect_to_vec(&std::fs::read(args.usercmds).unwrap());
        for (i, u) in usercmds.iter().enumerate() {
            project.clip.set_usercmd(i, *u);
        }
        project.clip.len = usercmds.len();
        project.clip.optimize();

        let mut buf = fs.read(&project.bsp).unwrap();
        Map::instance().load(project.bsp.to_str().unwrap(), &mut buf);

        let session = Session::new(&fs, project);

        let mut renderer = Renderer::new(gl);
        renderer.load_bsp(&fs, &args.bsp);

        let duration = (usercmds.len() - 1) as f32 * 0.008;
        let timeline = Timeline::new((0.0..=duration).into());

        Self {
            session,
            renderer: Arc::new(Mutex::new(renderer)),
            timeline,
            flycam: Default::default(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
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
                    &mut self.session,
                );
            }
            Tab::FlyCam => {
                self.flycam.ui(ui, Arc::clone(&self.renderer));
            }
            Tab::PlayerState => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.take_available_space();
                    ui.label(format!("{:#?}", self.session.game.ps()));
                });
            }
            Tab::Timeline => {
                self.session.with_project_mut(|project| {
                    clip_editor_ui(ui, &mut self.timeline, &mut project.clip);
                });
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

        let dock_state =
            eframe::get_value(cc.storage.unwrap(), eframe::APP_KEY).unwrap_or_else(|| {
                let mut dock_state = egui_dock::DockState::new(vec![Tab::Timeline]);

                let [_, ps] = dock_state.main_surface_mut().split_above(
                    egui_dock::NodeIndex::root(),
                    0.5,
                    vec![Tab::PlayerState],
                );

                let [_, fly] =
                    dock_state
                        .main_surface_mut()
                        .split_right(ps, 0.125, vec![Tab::FlyCam]);

                let [_, _] =
                    dock_state
                        .main_surface_mut()
                        .split_right(fly, 0.5, vec![Tab::FirstPerson]);

                dock_state
            });

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
            .update(&self.app_state.session);

        // Space is the standard play/pause key, but it's also jump, so enter also works even
        // during recording.
        if ctx.input(|i| {
            !self.app_state.timeline.recording && i.key_pressed(egui::Key::Space)
                || i.key_pressed(egui::Key::Enter)
        }) {
            self.app_state.timeline.playing = !self.app_state.timeline.playing;
        }

        if self.app_state.timeline.recording {
            self.app_state.session.disable_snapshot_worker();
        } else {
            self.app_state.session.enable_snapshot_worker();
        }

        self.app_state.timeline.update(ctx.input(|i| i.unstable_dt));

        if self.app_state.timeline.recording && self.app_state.timeline.playing {
            if self.app_state.timeline.playhead >= self.app_state.timeline.max_range.max - 1.0 {
                self.app_state.timeline.max_range.max += 1.0;
                self.app_state
                    .session
                    .with_project_mut(|p| p.clip.len += 125);
            }
            if self.app_state.timeline.playhead + 1.0 > self.app_state.timeline.visible_range.max {
                let delta = self.app_state.timeline.playhead + 1.0
                    - self.app_state.timeline.visible_range.max;
                if self.app_state.timeline.visible_range.span() >= 10.0 {
                    self.app_state.timeline.visible_range.min += delta;
                }
                self.app_state.timeline.visible_range.max += delta;
            }
        }

        self.app_state.timeline.last_valid_time =
            self.app_state.session.num_frames_with_valid_snapshot() as f32 * 0.008;

        if ctx.input(|i| i.key_pressed(egui::Key::O)) {
            eprintln!("optimizing curves");
            self.app_state.session.with_project_mut(|project| {
                project.clip.optimize();
            });
        }

        egui_dock::DockArea::new(&mut self.dock_state)
            .show_close_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show_leaf_collapse_buttons(false)
            .show(ctx, &mut self.app_state);

        self.app_state.session.seek(self.app_state.timeline.frame());
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.dock_state);
    }
}
