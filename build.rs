use std::{env, path::PathBuf};

#[derive(Debug)]
struct Callbacks;

impl bindgen::callbacks::ParseCallbacks for Callbacks {
    fn add_derives(&self, info: &bindgen::callbacks::DeriveInfo<'_>) -> Vec<String> {
        if info.name == "cplane_s"
            || info.name == "playerState_s"
            || info.name == "qboolean"
            || info.name == "trace_t"
            || info.name == "usercmd_s"
            || info.name == "vmCvar_t"
        {
            vec![
                "bytemuck::Pod".to_string(),
                "bytemuck::Zeroable".to_string(),
            ]
        } else {
            vec![]
        }
    }
}

fn main() {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    cc::Build::new()
        .files([
            "src/q3/cm_load.c",
            "src/q3/cm_patch.c",
            "src/q3/cm_polylib.c",
            "src/q3/cm_test.c",
            "src/q3/cm_trace.c",
            "src/q3/common.c",
            "src/q3/q_math.c",
            "src/q3/q_shared.c",
        ])
        .warnings(false)
        .compile("q3");

    bindgen::Builder::default()
        .header("src/q3/q_shared.h")
        .header("src/q3/qcommon.h")
        .header("src/q3/g_public.h")
        .header("src/q3/vm_local.h")
        .allowlist_function("Com_Init")
        .allowlist_function("COM_Parse")
        .allowlist_function("CM_LoadMap")
        .allowlist_function("CM_EntityString")
        .allowlist_function("CM_BoxTrace")
        .allowlist_type("gameImport_t")
        .allowlist_type("gameExport_t")
        .allowlist_type("playerState_t")
        .allowlist_type("sharedTraps_t")
        .allowlist_type("usercmd_t")
        .allowlist_type("vmCvar_t")
        .allowlist_type("opcode_t")
        .constified_enum_module("gameImport_t")
        .constified_enum_module("gameExport_t")
        .constified_enum_module("sharedTraps_t")
        .constified_enum_module("opcode_t")
        .parse_callbacks(Box::new(Callbacks))
        .generate()
        .unwrap()
        .write_to_file(out_path.join("bindings.rs"))
        .unwrap();

    println!("cargo::rerun-if-changed=src/q3");
}
