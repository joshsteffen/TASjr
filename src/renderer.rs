use std::{path::Path, sync::Arc};

use binrw::BinRead;
use eframe::{egui, glow};
use three_d::*;

use crate::{
    bsp::{Bsp, MapSurfaceType},
    fs::Fs,
};

pub struct Renderer {
    context: Context,
    map_model: Option<Gm<Mesh, NormalMaterial>>,
}

impl Renderer {
    pub fn new(gl: Arc<glow::Context>) -> Self {
        let context = Context::from_gl_context(gl).unwrap();
        Self {
            context,
            map_model: None,
        }
    }

    pub fn load_bsp(&mut self, fs: &Fs, path: impl AsRef<Path>) {
        let mut positions = vec![];
        let mut indices = vec![];
        {
            let mut f = fs.open(path).unwrap();
            let bsp = Bsp::read(&mut f).unwrap();
            let draw_verts = bsp.draw_verts.read(&mut f).unwrap();
            let draw_indexes = bsp.draw_indexes.read(&mut f).unwrap();
            for surface in bsp.surfaces.read(&mut f).unwrap() {
                match surface.surface_type {
                    MapSurfaceType::Planar | MapSurfaceType::TriangleSoup => {
                        let first_out_vert = positions.len() as u32;
                        for i in 0..surface.num_verts {
                            let vert = &draw_verts[(surface.first_vert + i) as usize];
                            positions.push(Vec3::from(vert.xyz));
                        }
                        for i in 0..surface.num_indexes {
                            indices.push(
                                draw_indexes[(surface.first_index + i) as usize] + first_out_vert,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut mesh = CpuMesh {
            positions: Positions::F32(positions),
            indices: Indices::U32(indices),
            ..Default::default()
        };
        mesh.compute_normals();
        self.map_model = Some(Gm::new(
            Mesh::new(&self.context, &mesh),
            NormalMaterial::default(),
        ));
    }

    pub fn render(&self, info: egui::PaintCallbackInfo, origin: Vec3, angles: Vec3) {
        let screen = RenderTarget::screen(
            &self.context,
            info.screen_size_px[0],
            info.screen_size_px[1],
        );

        let viewport = Viewport {
            x: info.viewport_in_pixels().left_px,
            y: info.viewport_in_pixels().from_bottom_px,
            width: info.viewport_in_pixels().width_px as u32,
            height: info.viewport_in_pixels().height_px as u32,
        };

        let scissor_box = ScissorBox::from(viewport);

        let dir = Mat3::from_angle_z(degrees(angles[1]))
            * Mat3::from_angle_y(degrees(angles[0]))
            * Vec3::unit_x();

        let camera = Camera::new_perspective(
            viewport,
            origin,
            origin + dir,
            Vec3::unit_z(),
            degrees(90.0),
            1.0,
            100000.0,
        );

        if let Some(map_model) = self.map_model.as_ref() {
            screen
                .clear_partially(
                    scissor_box,
                    ClearState::color_and_depth(0.0, 0.0, 0.0, 1.0, 1.0),
                )
                .render_partially(scissor_box, &camera, map_model, &[]);
        }
    }
}
