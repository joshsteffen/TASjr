use std::{collections::HashSet, ffi::CStr, path::PathBuf};

use binrw::BinRead;
use clap::Parser;
use three_d::*;

use qvm::{
    fs::Fs,
    game::Game,
    q3::{CM_EntityString, CM_LoadMap, COM_Parse, Com_Init, playerState_t},
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

fn main() {
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
    game.g_init(0, 0, false);
    game.g_run_frame(0);
    game.g_client_connect(0, true, false).unwrap();
    game.g_client_begin(0);
    let mut t = 8;

    let window = Window::new(Default::default()).unwrap();
    let context = window.gl();

    let mut camera = Camera::new_perspective(
        window.viewport(),
        vec3(0.0, 0.0, 5000.0),
        Vec3::zero(),
        Vec3::unit_y(),
        degrees(60.0),
        1.0,
        100000.0,
    );

    let mut positions = vec![];
    let mut indices = vec![];
    {
        let mut f = fs.open(&args.bsp).unwrap();
        let bsp = qvm::bsp::Bsp::read(&mut f).unwrap();
        let draw_verts = bsp.draw_verts.read(&mut f).unwrap();
        let draw_indexes = bsp.draw_indexes.read(&mut f).unwrap();
        for surface in bsp.surfaces.read(&mut f).unwrap() {
            match surface.surface_type {
                qvm::bsp::MapSurfaceType::Planar | qvm::bsp::MapSurfaceType::TriangleSoup => {
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
    let map_model = Gm::new(Mesh::new(&context, &mesh), NormalMaterial::default());

    let mut keys = HashSet::new();

    window.render_loop(move |frame_input| {
        for event in &frame_input.events {
            match event {
                Event::KeyPress { kind, .. } => {
                    keys.insert(*kind);
                }
                Event::KeyRelease { kind, .. } => {
                    keys.remove(kind);
                }
                Event::MouseMotion {
                    button: Some(MouseButton::Left),
                    delta,
                    ..
                } => {
                    game.user_cmd.angles[1] -= (delta.0 * 100.0) as i32;
                    game.user_cmd.angles[0] += (delta.1 * 100.0) as i32;
                }
                _ => {}
            }
        }

        game.user_cmd.serverTime = t;
        game.user_cmd.forwardmove =
            127 * (keys.contains(&Key::W) as i8 - keys.contains(&Key::S) as i8);
        game.user_cmd.rightmove =
            127 * (keys.contains(&Key::D) as i8 - keys.contains(&Key::A) as i8);
        game.user_cmd.upmove =
            127 * (keys.contains(&Key::Space) as i8 - keys.contains(&Key::C) as i8);

        game.g_client_think(0);
        game.g_run_frame(t);
        t += 16;

        let ps = game.vm.cast_mem_mut::<playerState_t>(game.clients);
        let origin = Vec3::from(ps.origin) + vec3(0.0, 0.0, ps.viewheight as f32);
        let dir = Mat3::from_angle_z(degrees(ps.viewangles[1]))
            * Mat3::from_angle_y(degrees(ps.viewangles[0]))
            * Vec3::unit_x();
        camera.set_view(origin, origin + dir, Vec3::unit_z());

        camera.set_viewport(frame_input.viewport);

        frame_input
            .screen()
            .clear(three_d::ClearState::color_and_depth(
                0.0, 0.0, 0.0, 1.0, 1.0,
            ))
            .render(&camera, &map_model, &[]);
        three_d::FrameOutput::default()
    });
}
