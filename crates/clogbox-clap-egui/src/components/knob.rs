use crate::GetContextExtra;
use clogbox_clap::gui::GuiContext;
use clogbox_clap::params::{ParamChangeEvent, ParamChangeKind, ParamId};
use egui::emath::normalized_angle;
use egui::{Response, Ui, Vec2};
use std::f32::consts::TAU;

pub struct Knob<E: ParamId> {
    pub id: E,
    pub knob_size: f32,
    guictx: GuiContext<E>,
    cur_value_normalized: f32,
}

impl<E: ParamId> Knob<E> {
    pub fn new(ctx: &egui::Context, id: E) -> Self {
        let gui_context = ctx.plugin_gui_context::<E>();
        let cur_value_normalized = gui_context.params[id].get_normalized();
        Self {
            id,
            knob_size: 40.0,
            guictx: gui_context.clone(),
            cur_value_normalized,
        }
    }

    pub fn with_knob_size(mut self, size: f32) -> Self {
        self.knob_size = size;
        self
    }
}

impl<E: ParamId> egui::Widget for Knob<E> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let knob_size = Vec2::splat(self.knob_size);

        let (rect, mut response) = ui.allocate_exact_size(knob_size, egui::Sense::click_and_drag());
        let visuals = ui.style().interact(&response);

        let side_length = rect.width().min(rect.height());
        let center = rect.center();

        // Fill background
        ui.painter().circle_filled(center, side_length / 2.0, visuals.bg_fill);
        ui.painter().circle_stroke(center, side_length / 2.0, visuals.bg_stroke);

        // Foreground tick
        let angle = angle_for_normalized_value(self.cur_value_normalized);
        let dir = angled_vec2(angle);
        let start = rect.center() + 0.25 * side_length * dir;
        let end = rect.center() + 0.45 * side_length * dir;
        ui.painter().line_segment([start, end], visuals.fg_stroke);

        if response.clicked() {
            self.gesture_begin();
            self.gesture_end();
        }
        if response.double_clicked() {
            let default_value = self.id.default_value();
            self.cur_value_normalized = self.id.mapping().normalize(default_value);
            response.mark_changed();
        }

        if response.drag_started() {
            self.gesture_begin();
        }
        if response.dragged() {
            let delta = response.drag_delta().y;
            let shift_pressed = ui.input(|input| input.modifiers.shift);
            if delta.abs() > 1e-4 {
                const DEFAULT_STEP: f32 = -0.005;
                let step = if shift_pressed {
                    0.01 * DEFAULT_STEP
                } else {
                    DEFAULT_STEP
                };
                let new_value = self.cur_value_normalized + delta * step;
                let new_value = new_value.clamp(0.0, 1.0);
                self.cur_value_normalized = new_value;
                response.mark_changed();
            }
        }
        if response.drag_stopped() {
            self.gesture_end();
        }

        if response.changed() {
            self.param_changed(self.cur_value_normalized);
        }

        response
    }
}

impl<E: ParamId> Knob<E> {
    fn gesture_begin(&self) {
        self.guictx.notifier.notify(ParamChangeEvent {
            id: self.id,
            kind: ParamChangeKind::GestureBegin,
        });
    }

    fn gesture_end(&self) {
        self.guictx.notifier.notify(ParamChangeEvent {
            id: self.id,
            kind: ParamChangeKind::GestureEnd,
        });
    }

    fn param_changed(&self, normalized: f32) {
        let value = self.id.mapping().denormalize(normalized);
        self.guictx.params.set(self.id, value);
        self.guictx.notifier.notify(ParamChangeEvent {
            id: self.id,
            kind: ParamChangeKind::ValueChange(value),
        });
    }
}

fn angle_for_normalized_value(normalized_value: f32) -> f32 {
    let turn_pc = 0.375 + 0.75 * normalized_value;
    normalized_angle(TAU * turn_pc)
}

fn angled_vec2(angle: f32) -> Vec2 {
    Vec2::new(angle.cos(), angle.sin())
}
