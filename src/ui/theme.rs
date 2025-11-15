use std::sync::Arc;

use eframe::egui;

fn set_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "Figtree-Regular".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../../fonts/Figtree-Regular.ttf"
        ))),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Figtree-Regular".to_owned());

    ctx.set_fonts(fonts);
}

fn set_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_corner_radius = egui::CornerRadius::ZERO;
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.window_fill = egui::Color32::from_gray(16);
    visuals.window_stroke.color = egui::Color32::from_gray(48);
    visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_gray(150);
    visuals.widgets.noninteractive.bg_stroke.color = egui::Color32::from_gray(56);
    ctx.set_visuals_of(egui::Theme::Dark, visuals);
}

pub fn set_theme(ctx: &egui::Context) {
    set_fonts(ctx);
    set_visuals(ctx);
}
