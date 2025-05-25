use crate::GuiContext;
use clogbox_clap::main_thread::Plugin;
use clogbox_clap::params::{ParamChangeEvent, ParamChangeKind, ParamId};
use std::f32::consts::TAU;
use std::marker::PhantomData;
use vizia::prelude::*;
use vizia::vg;

pub enum KnobEvent {
    Reset,
    SetValueNormalized(f32),
}

pub struct Knob<P: Plugin> {
    __plugin: PhantomData<P>,
    param: P::Params,
    normalized_value: f32,
}

impl<P: Plugin> Knob<P> {
    pub fn new(cx: &mut Context, param: P::Params) -> Handle<Self> {
        let gui_context = cx.data::<GuiContext<P>>().unwrap();
        Self {
            __plugin: PhantomData,
            param,
            normalized_value: gui_context.params[param].get_normalized(),
        }
        .build(cx, |_| ())
        .corner_radius(Percentage(50.0))
        .background_color(Color::gray())
        .color(Color::white())
    }
}

impl<P: Plugin> View for Knob<P> {
    fn element(&self) -> Option<&'static str> {
        Some("knob")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|ev: &ParamChangeEvent<P::Params>, _| {
            if ev.id != self.param {
                return;
            }
            let ParamChangeKind::ValueChange(value) = ev.kind else {
                return;
            };
            if self.normalized_value == value {
                return;
            }
            self.normalized_value = value;
            cx.emit(KnobEvent::SetValueNormalized(value));
        });
        event.take(|ev: &KnobEvent, _| match ev {
            &KnobEvent::Reset => {
                let default = self.param.mapping().normalize(self.param.default_value());
                cx.emit_to(cx.current(), KnobEvent::SetValueNormalized(default));
            }
            &KnobEvent::SetValueNormalized(value) => {
                if value == self.normalized_value {
                    return;
                }
                self.normalized_value = value;
                let gui_context = cx.data::<GuiContext<P>>().unwrap();
                gui_context.params[self.param].set_normalized(value);
                gui_context.notifier.notify(ParamChangeEvent {
                    id: self.param,
                    kind: ParamChangeKind::ValueChange(value),
                });
                cx.needs_redraw();
            }
        });
        event.map(|ev: &WindowEvent, meta| {
            if meta.target != cx.current() {
                return;
            }
            match ev {
                WindowEvent::MouseDown(MouseButton::Left) => {
                    cx.data::<GuiContext<P>>().unwrap().notifier.notify(ParamChangeEvent {
                        id: self.param,
                        kind: ParamChangeKind::GestureBegin,
                    });
                    cx.capture();
                    meta.consume();
                }
                WindowEvent::MouseUp(MouseButton::Left) => {
                    cx.data::<GuiContext<P>>().unwrap().notifier.notify(ParamChangeEvent {
                        id: self.param,
                        kind: ParamChangeKind::GestureEnd,
                    });
                    cx.release();
                    meta.consume();
                }
                WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                    cx.emit(ParamChangeEvent {
                        id: self.param,
                        kind: ParamChangeKind::GestureBegin,
                    });
                    cx.emit(KnobEvent::Reset);
                    cx.emit(ParamChangeEvent {
                        id: self.param,
                        kind: ParamChangeKind::GestureEnd,
                    });
                }
                _ => {}
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &Canvas) {
        let bounds = cx.bounds();
        let side_length = bounds.width().min(bounds.height());
        let center = bounds.center();

        let bg_color = vg::Color4f::from(cx.background_color());
        let bg_paint = vg::Paint::new(bg_color, None);

        let fg_color = vg::Color4f::from(cx.font_color());
        let fg_paint = vg::Paint::new(fg_color, None);

        cx.draw_shadows(canvas);

        // Background
        canvas.draw_circle(center, side_length / 2.0, &bg_paint);

        // Foreground tick
        let center = vg::Point::new(center.0, center.1);
        let angle = angle_for_normalized_value(self.normalized_value);
        let dir = angled_vec2(angle);
        let start = center + dir * 0.25 * side_length;
        let end = center + dir * 0.45 * side_length;
        canvas.draw_line(start, end, &fg_paint);
    }
}

fn angled_vec2(angle: f32) -> vg::Vector {
    vg::Vector::new(angle.cos(), angle.sin())
}

fn angle_for_normalized_value(normalized_value: f32) -> f32 {
    let turn_pc = 0.375 + 0.75 * normalized_value;
    normalized_angle(TAU * turn_pc)
}

fn normalized_angle(angle: f32) -> f32 {
    angle.rem_euclid(TAU)
}
