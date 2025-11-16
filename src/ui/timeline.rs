use eframe::egui::{
    Align2, FontId, Mesh, NumExt, Rangef, Rect, Response, Sense, Shape, Ui, pos2, remap,
    remap_clamp, vec2,
};

use crate::run::Run;

pub struct Timeline {
    pub visible_range: Rangef,
    pub max_range: Rangef,
    pub playhead: f32,
    pub playing: bool,
    pub recording: bool,
}

impl Timeline {
    pub fn new(max_range: Rangef) -> Self {
        Self {
            visible_range: max_range,
            max_range,
            playhead: 0.0,
            playing: false,
            recording: false,
        }
    }

    pub fn frame(&self) -> usize {
        (self.playhead * 1000.0) as usize / 8
    }

    pub fn update(&mut self, dt: f32) {
        if self.playing {
            self.playhead += dt;
            if self.playhead >= self.max_range.max {
                self.playhead = self.max_range.max;
                self.playing = false;
                self.recording = false;
            }
        }
    }

    pub fn show(&mut self, ui: &mut Ui, run: &Run) {
        let (id, _) = ui.allocate_space(vec2(ui.available_width(), 24.0));

        let rect = ui.max_rect();
        let response = ui.interact(rect, id, Sense::click_and_drag());

        self.interact(ui, &response);
        self.paint_ticks(ui, rect, run);
        self.paint_playhead(ui, rect, &response);
    }

    fn interact(&mut self, ui: &mut Ui, response: &Response) {
        let rect = response.rect;

        if response.is_pointer_button_down_on() {
            let pointer_pos = response.interact_pointer_pos().unwrap();
            self.playhead = remap_clamp(pointer_pos.x, rect.x_range(), self.visible_range);
        }

        let Some(pointer_pos) = response.hover_pos() else {
            return;
        };

        let zoom = ui.input(|i| i.smooth_scroll_delta.y) * 0.005;
        if zoom != 0.0 {
            let frac = remap(pointer_pos.x, rect.x_range(), 0.0..=1.0);
            let span = self.visible_range.span();
            let grow_by = (span * zoom).at_least(0.03125 - span);
            self.visible_range.min -= grow_by * frac;
            self.visible_range.max += grow_by * (1.0 - frac);
        }

        let scroll = ui.input(|i| i.smooth_scroll_delta.x);
        let scroll = scroll * self.visible_range.span() / rect.x_range().span();
        self.visible_range.min -= scroll;
        self.visible_range.max -= scroll;

        // Try to prevent exceeding max_range by shifting the whole timeline
        let mut shift = 0.0;
        if self.visible_range.min < self.max_range.min {
            shift = self.max_range.min - self.visible_range.min;
        } else if self.visible_range.max > self.max_range.max {
            shift = self.max_range.max - self.visible_range.max;
        }
        self.visible_range.min += shift;
        self.visible_range.max += shift;

        // Chop off the excess as a last resort
        self.visible_range = self.visible_range.intersection(self.max_range);
    }

    fn paint_ticks(&self, ui: &mut Ui, rect: Rect, run: &Run) {
        let divisions = [
            0.008, 0.08, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0, 1800.0, 3600.0,
        ];

        let mut division = *divisions
            .iter()
            .find(|&&d| d >= self.visible_range.span() / 15.0)
            .unwrap_or(divisions.last().unwrap());

        let num_subdivisions = if division > 0.008 { 10 } else { 1 };
        division /= num_subdivisions as f32;

        let first_tick = (self.visible_range.min / division).ceil() as i32;
        let last_tick = (self.visible_range.max / division).floor() as i32;

        let last_valid_time = run.num_frames_with_valid_snapshot() as f32 * 0.008;

        for tick in first_tick..=last_tick {
            let big_tick = tick % num_subdivisions == 0;
            let height = if big_tick { 12.0 } else { 8.0 };

            let t = tick as f32 * division;

            let stroke = if t < last_valid_time && big_tick {
                ui.visuals().widgets.noninteractive.fg_stroke
            } else {
                ui.visuals().widgets.noninteractive.bg_stroke
            };

            let x = remap(t, self.visible_range, rect.x_range());
            ui.painter()
                .vline(x, rect.top()..=rect.top() + height, stroke);

            if t > 0.0 && big_tick {
                ui.painter().text(
                    pos2(x + 2.0, rect.top() + 12.0),
                    Align2::CENTER_TOP,
                    format_time(t),
                    FontId::proportional(12.0),
                    stroke.color,
                );
            }
        }
    }

    fn paint_playhead(&self, ui: &mut Ui, rect: Rect, response: &Response) {
        if let Some(pointer_pos) = response.hover_pos()
            && rect.contains(pointer_pos)
        {
            ui.painter().vline(
                pointer_pos.x,
                rect.y_range(),
                ui.visuals().widgets.inactive.fg_stroke,
            );
        }

        let playhead_x = remap(self.playhead, self.visible_range, rect.x_range());

        ui.painter().vline(
            playhead_x,
            rect.y_range(),
            ui.visuals().widgets.active.bg_stroke,
        );

        let mut mesh = Mesh::default();
        let color = ui.visuals().widgets.active.fg_stroke.color;
        mesh.colored_vertex(pos2(playhead_x, rect.top() + 8.0), color);
        mesh.colored_vertex(pos2(playhead_x + 4.0, rect.top()), color);
        mesh.colored_vertex(pos2(playhead_x - 4.0, rect.top()), color);
        mesh.add_triangle(0, 1, 2);
        ui.painter().add(Shape::mesh(mesh));
    }
}

fn format_time(time: f32) -> String {
    let ms = (time * 1000.0).round() as u32;
    if ms < 60 * 1000 {
        format!("{}.{:03}", ms / 1000, ms % 1000)
    } else {
        format!(
            "{}:{:02}.{:03}",
            ms / (60 * 1000),
            (ms % (60 * 1000)) / 1000,
            ms % 1000
        )
    }
}
