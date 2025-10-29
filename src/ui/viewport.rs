use std::sync::{Arc, Mutex};

use eframe::egui;
use three_d::{InnerSpace, Mat3, Vec3, Zero, degrees, vec3};

use crate::{q3::angle_to_short, renderer::Renderer, run::Run};

const SENSITIVITY: f32 = 0.25;

fn viewport<F>(ui: &mut egui::Ui, f: F)
where
    F: Fn(egui::PaintCallbackInfo) + Send + Sync + 'static,
{
    ui.take_available_space();
    ui.painter().add(egui::PaintCallback {
        rect: ui.min_rect(),
        callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, _painter| {
            f(info);
        })),
    });
}

pub fn first_person_ui(
    ui: &mut egui::Ui,
    renderer: Arc<Mutex<Renderer>>,
    run: &mut Run,
    frame: usize,
) {
    let ps = run.game.ps();
    let origin = Vec3::from(ps.origin) + vec3(0.0, 0.0, ps.viewheight as f32);
    let viewangles = ps.viewangles;
    viewport(ui, move |info| {
        renderer
            .lock()
            .unwrap()
            .render(info, origin, viewangles.into());
    });

    if frame >= run.num_frames_with_valid_snapshot() {
        ui.painter().text(
            ui.min_rect().center(),
            egui::Align2::CENTER_CENTER,
            "🚫",
            egui::FontId::proportional(ui.min_size().min_elem()),
            egui::Color32::from_white_alpha(64),
        );
    }

    let angles_id = egui::Id::new("first_person_drag_angles");
    let last_frame_id = egui::Id::new("first_person_drag_last_frame");

    let response = ui.interact(
        ui.min_rect(),
        ui.id().with("first_person"),
        egui::Sense::drag(),
    );

    if response.drag_started() {
        ui.memory_mut(|mem| {
            mem.data.insert_temp(angles_id, viewangles);
            mem.data.insert_temp(last_frame_id, frame);
        });
    }

    if response.dragged() {
        let motion = response.drag_motion() * SENSITIVITY;
        ui.memory_mut(|mem| {
            let angles: &mut [f32; 3] = mem.data.get_temp_mut_or_default(angles_id);
            angles[1] -= motion.x;
            angles[0] += motion.y;
            let angles = angles.map(|x| angle_to_short(x) as i32);

            let last_frame = mem.data.get_temp_mut_or_default(last_frame_id);
            for i in *last_frame..=frame {
                run.with_usercmd_mut(i, |u| {
                    u.angles = angles;
                })
            }
            *last_frame = frame;
        });
    }
}

pub struct FlyCam {
    position: Vec3,
    angles: Vec3,
}

impl Default for FlyCam {
    fn default() -> Self {
        Self {
            position: Vec3::zero(),
            angles: Vec3::zero(),
        }
    }
}

impl FlyCam {
    pub fn ui(&mut self, ui: &mut egui::Ui, renderer: Arc<Mutex<Renderer>>) {
        let (position, angles) = (self.position, self.angles);
        viewport(ui, move |info| {
            renderer.lock().unwrap().render(info, position, angles);
        });

        let response = ui.interact(ui.min_rect(), ui.id(), egui::Sense::drag());
        if response.dragged() {
            let motion = response.drag_motion() * SENSITIVITY;
            self.angles.y -= motion.x;
            self.angles.x += motion.y;
            self.angles.x = self.angles.x.clamp(-89.9, 89.9);
        }

        if response.is_pointer_button_down_on() {
            response.request_focus();
        }

        if response.has_focus() {
            ui.input(|i| {
                use egui::Key::*;
                let forward = i.key_down(W) as i8 - i.key_down(S) as i8;
                let right = i.key_down(D) as i8 - i.key_down(A) as i8;
                let dir = vec3(forward as f32, -right as f32, 0.0);
                if !dir.is_zero() {
                    let mat = Mat3::from_angle_z(degrees(self.angles[1]))
                        * Mat3::from_angle_y(degrees(self.angles[0]));
                    self.position += mat * dir.normalize() * i.stable_dt * 1000.0;
                }
            });
        }
    }
}
