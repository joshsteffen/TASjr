use std::{ffi::CStr, path::PathBuf};

use clap::Parser;
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

    let mut buf = fs.read(args.bsp).unwrap();
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
    game.g_init(0, 0, false);
    game.g_run_frame(0);
    game.g_client_connect(0, true, false).unwrap();
    game.g_client_begin(0);
    let start = std::time::Instant::now();
    let mut t = 8;
    while t < 50000 {
        let ps = game.vm.cast_mem_mut::<playerState_t>(game.clients);
        println!("{} {} {}", ps.origin[0], ps.origin[1], ps.origin[2]);

        game.user_cmd.serverTime = t;
        game.user_cmd.forwardmove = 127;
        game.g_client_think(0);
        game.g_run_frame(t);
        t += 8;
    }
    let end = start.elapsed();
    eprintln!("{end:?}");
}
