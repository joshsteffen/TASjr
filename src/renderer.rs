use std::{
    ops::{Index, IndexMut},
    path::Path,
    sync::Arc,
};

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
                    MapSurfaceType::Patch => {
                        let (first_vert, width, height) = (
                            surface.first_vert as usize,
                            surface.patch_width as usize,
                            surface.patch_height as usize,
                        );

                        let mut points = Grid::new(width, height);
                        for i in 0..width {
                            for j in 0..height {
                                let vert = &draw_verts[first_vert + i + j * width];
                                points[(i, j)] = Vec3::from(vert.xyz);
                            }
                        }

                        points = tessellate_bezier(points, 16, |a, b, t| Vec3::lerp(*a, *b, t));

                        let first_vert = positions.len() as u32;
                        let (patch_vertices, patch_indices) = points.triangulate();
                        positions.extend_from_slice(&patch_vertices);
                        indices.extend(patch_indices.iter().map(|i| first_vert + i));
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

pub struct Grid<T> {
    size: (usize, usize),
    stride: (usize, usize),
    points: Vec<T>,
}

impl<T: Clone + Zero> Grid<T> {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            size: (width, height),
            stride: (1, width),
            points: vec![T::zero(); width * height],
        }
    }

    pub fn transpose(&mut self) {
        self.size = (self.size.1, self.size.0);
        self.stride = (self.stride.1, self.stride.0);
    }

    pub fn triangulate(self) -> (Vec<T>, Vec<u32>) {
        let to_index = |i, j| (i * self.stride.0 + j * self.stride.1) as u32;
        let mut indices = vec![];
        for x in 0..self.size.0 - 1 {
            for y in 0..self.size.1 - 1 {
                indices.push(to_index(x, y));
                indices.push(to_index(x, y + 1));
                indices.push(to_index(x + 1, y + 1));

                indices.push(to_index(x + 1, y + 1));
                indices.push(to_index(x + 1, y));
                indices.push(to_index(x, y));
            }
        }
        (self.points, indices)
    }
}

impl<T> Index<(usize, usize)> for Grid<T> {
    type Output = T;

    fn index(&self, index: (usize, usize)) -> &Self::Output {
        &self.points[index.0 * self.stride.0 + index.1 * self.stride.1]
    }
}

impl<T> IndexMut<(usize, usize)> for Grid<T> {
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        &mut self.points[index.0 * self.stride.0 + index.1 * self.stride.1]
    }
}

pub fn tessellate_bezier<T, F>(mut points: Grid<T>, steps: usize, lerp_vertices: F) -> Grid<T>
where
    T: Clone + Zero,
    F: Fn(&T, &T, f32) -> T,
{
    let interpolate_bezier = |a: &T, b: &T, c: &T, t: f32| {
        lerp_vertices(&lerp_vertices(a, b, t), &lerp_vertices(b, c, t), t)
    };

    for _ in 0..2 {
        let controls = points;

        let (curves_per_row, rows) = (controls.size.0 / 2, controls.size.1);
        points = Grid::new(curves_per_row * (steps - 1) + 1, rows);

        for y in 0..controls.size.1 {
            let mut out_x = 0;
            for x in (0..controls.size.0 - 1).step_by(2) {
                let (a, b, c) = (
                    &controls[(x, y)],
                    &controls[(x + 1, y)],
                    &controls[(x + 2, y)],
                );

                for k in 0..steps {
                    let t = k as f32 / (steps - 1) as f32;
                    points[(out_x, y)] = interpolate_bezier(a, b, c, t);
                    out_x += 1;
                }
                out_x -= 1;
            }
        }

        points.transpose();
    }

    points
}
