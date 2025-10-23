use std::sync::{Arc, Mutex};

use eframe::egui;
use three_d::{InnerSpace, Mat3, Vec3, Zero, degrees, vec3};

use crate::{q3::playerState_t, renderer::Renderer};

fn viewport<F>(ui: &mut egui::Ui, f: F)
where
    F: Fn(egui::PaintCallbackInfo) + Send + Sync + 'static,
{
    egui::Frame::NONE.show(ui, |ui| {
        ui.take_available_space();
        ui.painter().add(egui::PaintCallback {
            rect: ui.min_rect(),
            callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, _painter| {
                f(info);
            })),
        })
    });
}

pub fn first_person_ui(ui: &mut egui::Ui, renderer: Arc<Mutex<Renderer>>, ps: &playerState_t) {
    let origin = Vec3::from(ps.origin) + vec3(0.0, 0.0, ps.viewheight as f32);
    let angles = ps.viewangles.into();
    viewport(ui, move |info| {
        renderer.lock().unwrap().render(info, origin, angles, false);
    });
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
            renderer
                .lock()
                .unwrap()
                .render(info, position, angles, true);
        });

        let response = ui.interact(ui.min_rect(), ui.id(), egui::Sense::drag());
        if response.dragged() {
            self.angles.y -= response.drag_motion().x * 0.25;
            self.angles.x += response.drag_motion().y * 0.25;
            self.angles.x = self.angles.x.clamp(-89.9, 89.9);
        }

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
