use std::{
    ops::{Index, IndexMut},
    path::Path,
    sync::Arc,
};

use binrw::BinRead;
use bytemuck::cast_slice;
use eframe::{egui, glow};
use three_d::*;

use crate::{
    bsp::{Bsp, LIGHTMAP_SIZE, Lightmap, MapSurfaceType},
    fs::Fs,
    run::Run,
};

pub struct Renderer {
    context: Context,
    map_model: Option<Gm<Mesh, PhysicalMaterial>>,
    bounding_box_model: Gm<InstancedMesh, PhysicalMaterial>,
}

impl Renderer {
    pub fn new(gl: Arc<glow::Context>) -> Self {
        let context = Context::from_gl_context(gl).unwrap();

        let mut material = PhysicalMaterial::new_transparent(
            &context,
            &CpuMaterial {
                albedo: Srgba::new(255, 64, 64, 128),
                ..Default::default()
            },
        );
        material.render_states.cull = Cull::Back;
        let bounding_box_model = Gm::new(
            InstancedMesh::new(&context, &Instances::default(), &CpuMesh::cube()),
            material,
        );

        Self {
            context,
            map_model: None,
            bounding_box_model,
        }
    }

    pub fn load_bsp(&mut self, fs: &Fs, path: impl AsRef<Path>) {
        let mut f = fs.open(path).unwrap();
        let bsp = Bsp::read(&mut f).unwrap();
        let draw_verts = bsp.draw_verts.read(&mut f).unwrap();
        let draw_indexes = bsp.draw_indexes.read(&mut f).unwrap();

        let mut lightmaps = bsp.lightmaps.read(&mut f).unwrap();

        let white_lightmap = lightmaps.len();
        lightmaps.push(Lightmap {
            pixels: [[[255; _]; _]; _],
        });

        let mut positions = vec![];
        let mut indices = vec![];
        let mut uvs = vec![];
        let mut colors = vec![];

        for surface in bsp.surfaces.read(&mut f).unwrap() {
            let lightmap_num = if surface.lightmap_num < 0 {
                white_lightmap
            } else {
                surface.lightmap_num as usize
            };

            let lightmap_base_uv = vec2(0.0, lightmap_num as f32);

            match surface.surface_type {
                MapSurfaceType::Planar | MapSurfaceType::TriangleSoup => {
                    let first_out_vert = positions.len() as u32;

                    for i in 0..surface.num_verts {
                        let vert = &draw_verts[(surface.first_vert + i) as usize];
                        positions.push(Vec3::from(vert.xyz));
                        uvs.push(lightmap_base_uv + Vec2::from(vert.lightmap));
                        colors.push(if surface.lightmap_num < 0 {
                            Srgba::from(vert.color)
                        } else {
                            Srgba::WHITE
                        });
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
                            points[(i, j)] = Vertex {
                                position: Vec3::from(vert.xyz),
                                uv: lightmap_base_uv + Vec2::from(vert.lightmap),
                            }
                        }
                    }

                    points = tessellate_bezier(points, 16, Vertex::lerp);

                    let first_vert = positions.len() as u32;
                    let (patch_vertices, patch_indices) = points.triangulate();
                    for vertex in patch_vertices {
                        positions.push(vertex.position);
                        uvs.push(vertex.uv);
                        colors.push(Srgba::WHITE);
                    }
                    indices.extend(patch_indices.iter().map(|i| first_vert + i));
                }
                _ => {}
            }
        }

        uvs.iter_mut().for_each(|uv| uv.y /= lightmaps.len() as f32);

        let mut mesh = CpuMesh {
            positions: Positions::F32(positions),
            indices: Indices::U32(indices),
            uvs: Some(uvs),
            colors: Some(colors),
            ..Default::default()
        };
        mesh.compute_normals();

        let (width, height) = (LIGHTMAP_SIZE, lightmaps.len() * LIGHTMAP_SIZE);
        let lightmap = Texture2D::new_empty::<[u8; 3]>(
            &self.context,
            width as u32,
            height as u32,
            Interpolation::Linear,
            Interpolation::Linear,
            None,
            Wrapping::ClampToEdge,
            Wrapping::ClampToEdge,
        );
        lightmap.fill::<[u8; 3]>(cast_slice(&lightmaps));

        let mut material = PhysicalMaterial {
            albedo_texture: Some(Texture2DRef::from_texture(lightmap)),
            ..Default::default()
        };
        material.render_states.cull = Cull::Front;
        self.map_model = Some(Gm::new(Mesh::new(&self.context, &mesh), material));
    }

    pub fn update(&mut self, run: &Run) {
        let g_entities = run.game.g_entities.unwrap();

        let mut instances = Instances {
            transformations: vec![Mat4::identity(); g_entities.count as usize],
            ..Default::default()
        };

        for i in 0..g_entities.count {
            let ent = run.game.entity(i);

            let origin: Vec3 = ent.r.currentOrigin.into();
            let mins: Vec3 = origin + Vec3::from(ent.r.mins);
            let maxs: Vec3 = origin + Vec3::from(ent.r.maxs);

            if mins != maxs {
                let center = (mins + maxs) * 0.5;
                let size = (maxs - mins) * 0.5;

                instances.transformations[i as usize] =
                    Mat4::from_translation(center) * Mat4::from_diagonal(size.extend(1.0));
            }
        }

        self.bounding_box_model.set_instances(&instances);
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

        let alight = AmbientLight::new(&self.context, 0.35, Srgba::WHITE);
        let dlight1 = DirectionalLight::new(
            &self.context,
            1.0,
            Srgba::new_opaque(255, 255, 240),
            vec3(1.0, 2.0, -3.0),
        );
        let dlight2 = DirectionalLight::new(
            &self.context,
            1.0,
            Srgba::new_opaque(180, 180, 192),
            vec3(-2.0, -1.0, -3.0),
        );

        if let Some(map_model) = self.map_model.as_ref() {
            screen
                .clear_partially(
                    scissor_box,
                    ClearState::color_and_depth(0.0, 0.0, 0.0, 1.0, 1.0),
                )
                .render_partially(
                    scissor_box,
                    &camera,
                    map_model.into_iter().chain(&self.bounding_box_model),
                    &[&alight, &dlight1, &dlight2],
                );
        }
    }
}

#[derive(Clone)]
struct Vertex {
    position: Vec3,
    uv: Vec2,
}

impl Vertex {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        Self {
            position: Vec3::lerp(a.position, b.position, t),
            uv: Vec2::lerp(a.uv, b.uv, t),
        }
    }
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            position: Vec3::zero(),
            uv: Vec2::zero(),
        }
    }
}

struct Grid<T> {
    size: (usize, usize),
    stride: (usize, usize),
    points: Vec<T>,
}

impl<T: Clone + Default> Grid<T> {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            size: (width, height),
            stride: (1, width),
            points: vec![T::default(); width * height],
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

fn tessellate_bezier<T, F>(mut points: Grid<T>, steps: usize, lerp_vertices: F) -> Grid<T>
where
    T: Clone + Default,
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
