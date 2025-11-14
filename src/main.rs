use tasjr::ui::App;

fn main() -> eframe::Result {
    eframe::run_native(
        "TASjr",
        eframe::NativeOptions {
            depth_buffer: 24,
            multisampling: 8,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
