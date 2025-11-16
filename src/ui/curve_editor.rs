use eframe::{
    egui::{Color32, PointerButton, Pos2, Rect, Sense, Ui, pos2},
    emath::RectTransform,
    epaint::Hsva,
};

use crate::animation::{Curve, Interpolation, Keyframe};

pub fn curve_editor_ui(ui: &mut Ui, range: Rect, curve: &mut Curve, color: Color32) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::empty());
    let to_screen = RectTransform::from_to(range, response.rect.shrink(4.0));

    let mut last_point =
        to_screen.transform_pos(pos2(range.left(), curve.eval_smooth(range.left())));

    let mut interpolation = Interpolation::Hold;

    for keyframe in
        curve.keyframes_affecting_range(range.left() as usize..=range.right().ceil() as usize)
    {
        let point = to_screen.transform_pos(pos2(keyframe.time as f32, keyframe.value as f32));

        if point.x > last_point.x {
            let end = match interpolation {
                Interpolation::Hold => pos2(point.x, last_point.y),
                Interpolation::Linear => point,
            };
            painter.line_segment([last_point, pos2(end.x + 1.0, end.y)], (2.0, color));
        }

        if to_screen.scale().x >= 2.0 {
            painter.circle_filled(point, 4.0, color);
        }

        last_point = point;
        interpolation = keyframe.interpolation;
    }

    if last_point.x < response.rect.right() {
        painter.line_segment(
            [last_point, pos2(response.rect.right(), last_point.y)],
            (2.0, color),
        );
    }

    #[derive(Clone, Copy, Default)]
    struct State {
        dragging: Option<usize>,
    }

    let mut state: State = ui.data_mut(|data| *data.get_temp_mut_or_default(response.id));

    let interaction_time = |pointer: Pos2| -> Option<usize> {
        let pointer_time = to_screen.inverse().transform_pos(pointer).x.round() as usize;
        let prev_time = curve.prev_keyframe(pointer_time).map(|k| k.time);
        let next_time = curve.next_keyframe(pointer_time).map(|k| k.time);
        for t in [prev_time, next_time, Some(pointer_time)]
            .iter()
            .filter_map(|&t| t)
        {
            let value = curve.eval_smooth(t as f32);
            let screen_point = to_screen.transform_pos(pos2(t as f32, value));
            if pointer.distance_sq(screen_point) < 100.0 {
                return Some(t);
            }
        }
        None
    };

    // We only want to steal mouse inputs if the user is actually interacting with the curve,
    // otherwise they pass through to the timeline.
    let mut interacting = state.dragging.is_some();
    if !interacting && let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) {
        interacting = interaction_time(pointer).is_some();
    }
    if !interacting && let Some(pointer) = ui.input(|i| i.pointer.press_origin()) {
        interacting = interaction_time(pointer).is_some();
    }
    if !interacting {
        return;
    }

    let response = response.interact(Sense::click_and_drag());

    if let Some(highlight_time) = state
        .dragging
        .or_else(|| response.hover_pos().and_then(interaction_time))
    {
        let mut color = Hsva::from(color);
        color.s *= 0.8;
        color.v *= 4.0;
        let point = to_screen.transform_pos(pos2(
            highlight_time as f32,
            curve.eval_smooth(highlight_time as f32),
        ));
        ui.painter().circle_filled(point, 4.5, color);
    }

    if response.double_clicked()
        && let Some(pointer) = response.interact_pointer_pos()
        && let Some(time) = interaction_time(pointer)
    {
        curve.remove_keyframe(time);
        return;
    }

    if response.clicked_by(PointerButton::Secondary)
        && let Some(pointer) = response.interact_pointer_pos()
        && let Some(time) = interaction_time(pointer)
    {
        if let Some(keyframe) = curve.keyframe_mut(time) {
            keyframe.interpolation = match keyframe.interpolation {
                Interpolation::Hold => Interpolation::Linear,
                Interpolation::Linear => Interpolation::Hold,
            };
        }
        return;
    }

    if response.drag_started()
        && let Some(click) = ui.input(|i| i.pointer.press_origin())
        && let Some(time) = interaction_time(click)
    {
        if curve.keyframe(time).is_none() {
            curve.insert_keyframe(Keyframe::new(
                time,
                curve.eval(time),
                curve
                    .prev_keyframe(time)
                    .map(|k| k.interpolation)
                    .unwrap_or(Interpolation::Hold),
            ));
        }
        state.dragging = Some(time);
    } else if response.drag_stopped() {
        state.dragging = None;
    }

    if response.dragged()
        && response.drag_motion().length_sq() > 0.0
        && let Some(pointer) = response.interact_pointer_pos()
        && let Some(dragging) = state.dragging
        && let Some(mut keyframe) = curve.remove_keyframe(dragging)
    {
        let min = curve
            .prev_keyframe(dragging)
            .map(|k| k.time as f32 + 1.0)
            .unwrap_or(0.0);
        let max = curve
            .next_keyframe(dragging)
            .map(|k| k.time as f32 - 1.0)
            .unwrap_or(f32::INFINITY);
        if min <= max {
            let p = to_screen.inverse().transform_pos(pointer);
            keyframe.time = p.x.round().clamp(min, max) as usize;
            keyframe.value = range.y_range().as_positive().clamp(p.y.round()) as isize;
            curve.insert_keyframe(keyframe);
            state.dragging = Some(keyframe.time);
        }
    }

    ui.data_mut(|data| data.insert_temp(response.id, state));
}
