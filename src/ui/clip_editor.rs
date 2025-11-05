use eframe::egui::{Rect, Ui, UiBuilder, ecolor::Hsva, vec2};

use crate::{
    project::clip::Clip,
    ui::{Timeline, curve_editor::curve_editor_ui},
};

pub fn clip_editor_ui(ui: &mut Ui, timeline: &mut Timeline, clip: &mut Clip) {
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.allocate_space(vec2(0.0, 24.0)); // TODO

            let mut track_header_rect = ui.available_rect_before_wrap();
            track_header_rect.set_height(track_header_rect.height() / clip.curves.len() as f32);

            for parameter in clip.curves.keys() {
                ui.scope_builder(UiBuilder::new().max_rect(track_header_rect), |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.label(parameter.name());
                    });
                });
                track_header_rect =
                    track_header_rect.translate(vec2(0.0, track_header_rect.height()));
            }
        });

        ui.vertical(|ui| {
            timeline.show(ui);

            let frame_range =
                timeline.visible_range.min * 125.0..=timeline.visible_range.max * 125.0;

            let n = clip.curves.len() as f32;

            let mut track_rect = ui.available_rect_before_wrap();
            track_rect.set_height(track_rect.height() / n);

            for (i, (parameter, curve)) in clip.curves.iter_mut().enumerate() {
                let color = Hsva::new(i as f32 / n, 0.9, 0.25, 1.0);
                let (min, max) = parameter.range();
                ui.scope_builder(UiBuilder::new().max_rect(track_rect), |ui| {
                    curve_editor_ui(
                        ui,
                        Rect::from_x_y_ranges(&frame_range, max as f32..=min as f32),
                        curve,
                        color.into(),
                    );
                });
                track_rect = track_rect.translate(vec2(0.0, track_rect.height()));
            }
        });
    });
}
