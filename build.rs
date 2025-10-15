fn main() {
    println!("cargo::rerun-if-changed=src/q3");
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
}
