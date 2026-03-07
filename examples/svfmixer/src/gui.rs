use crate::params::Param;
use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::PluginView;
use clogbox_clap::params::{ParamChangeEvent, ParamChangeKind, ParamId};
use clogbox_clap::PluginError;
use clogbox_clap_egui::egui::{emath, Color32, Context, Frame, Sense, Ui};
use clogbox_clap_egui::egui_baseview::Queue;
use clogbox_clap_egui::{egui, generic_ui, EguiPluginView, GetContextExtra};
use clogbox_filters::svf::FilterType;
use clogbox_math::linear_to_db;
use egui_plot::{GridMark, Legend, PlotResponse};
use nalgebra as na;
use nalgebra::ComplexField;
use std::f64::consts::TAU;
use std::fmt;
use std::fmt::Formatter;
use std::hash::Hash;

const WIDTH: f32 = 530.0;
const HEIGHT: f32 = 300.0;

const WINDOW_PADDING: f32 = 10.0;
const KNOBS_HEIGHT: f32 = 100.0;
const PLOT_HEIGHT: f32 = HEIGHT - KNOBS_HEIGHT;

struct Hertz(f64);

impl fmt::Display for Hertz {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.0 > 1e3 {
            fmt::Display::fmt(&(self.0 / 1e3), f)?;
            write!(f, " kHz")
        } else {
            fmt::Display::fmt(&self.0, f)?;
            write!(f, " Hz")
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct SvfFreqResponse {
    pub sample_rate: f64,
    pub cutoff: f64,
    pub resonance: f64,
    pub gain: f64,
    pub filter_type: FilterType,
}

impl SvfFreqResponse {
    fn from_params(ui: &mut Ui) -> Self {
        let gui_context = ui.plugin_gui_context::<Param>();
        Self {
            sample_rate: 2.0 * gui_context.sample_rate(), // Oversampled 2x in DSP
            cutoff: gui_context.params.get(Param::Cutoff) as _,
            resonance: gui_context.params.get(Param::Resonance) as _,
            gain: gui_context.params.get(Param::Gain) as _,
            filter_type: gui_context.params.get_enum(Param::FilterType),
        }
    }
    fn show(self, id: impl Hash, ui: &mut Ui) -> PlotResponse<()> {
        egui_plot::Plot::new(id)
            .width(WIDTH - 2.0 * WINDOW_PADDING)
            .height(PLOT_HEIGHT)
            .x_axis_label("Frequency")
            .x_grid_spacer(|input| {
                let (lmin, lmax) = input.bounds;
                let first = lmin.ceil() as usize;
                let last = lmax.floor() as usize;
                let mut res = Vec::new();
                for i in first..last {
                    res.push(GridMark {
                        value: i as _,
                        step_size: 100.0,
                    });
                    let sub = 10f64.powf(i as _);
                    for i in 2..=9 {
                        let tick = sub * i as f64;
                        res.push(GridMark {
                            value: tick,
                            step_size: 10.0,
                        });
                    }
                }
                res
            })
            .x_axis_formatter(|mark, _| format!("{:3.0}", Hertz(10.0.powf(mark.value))))
            .y_axis_label("Gain")
            .y_axis_formatter(|mark, _| format!("{:2.1} dB", mark.value))
            .auto_bounds(false)
            .default_x_bounds(20.0.log10(), 20e3.log10())
            .default_y_bounds(-30.0, 30.0)
            .legend(Legend::default())
            .allow_drag(false)
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_boxed_zoom(false)
            .view_aspect(2.0)
            .sense(Sense::drag())
            .cursor_color(Color32::TRANSPARENT)
            .show(ui, |ui| {
                let rect = ui.response().rect;
                ui.line(egui_plot::Line::new(
                    "Frequency",
                    egui_plot::PlotPoints::from_explicit_callback(
                        |log_f| linear_to_db(self.freq_response(10.0.powf(log_f)).norm()),
                        20.0.log10()..20e3.log10(),
                        rect.width() as _,
                    ),
                ));
                let filter_type = ui
                    .ctx()
                    .plugin_gui_context::<Param>()
                    .params
                    .get_enum::<FilterType>(Param::FilterType);
                let y_param = if matches!(
                    filter_type,
                    FilterType::Lowshelf | FilterType::Highshelf | FilterType::PeakShelf
                ) {
                    Param::Gain
                } else {
                    Param::Resonance
                };
                if ui.response().drag_started() {
                    let notifier = ui.ctx().plugin_gui_context::<Param>().notifier;
                    notifier.notify(ParamChangeEvent {
                        id: Param::Cutoff,
                        kind: ParamChangeKind::GestureBegin,
                    });
                    notifier.notify(ParamChangeEvent {
                        id: y_param,
                        kind: ParamChangeKind::GestureBegin,
                    });
                }
                if ui.response().dragged() {
                    let size = rect.size();
                    let params = ui.ctx().plugin_gui_context::<Param>().params;
                    let cutoff = params.get_normalized(Param::Cutoff);
                    let yvalue = params.get_normalized(y_param);
                    let dv = ui.response().drag_delta() / size;
                    let notifier = ui.ctx().plugin_gui_context::<Param>().notifier;
                    if dv.x.abs() > 1e-4 {
                        let cutoff = Param::Cutoff.mapping().denormalize(f32::clamp(cutoff + dv.x, 0.0, 1.0));
                        notifier.notify(ParamChangeEvent {
                            id: Param::Cutoff,
                            kind: ParamChangeKind::ValueChange(cutoff),
                        });
                    }
                    if dv.y.abs() > 1e-4 {
                        let yvalue = y_param.mapping().denormalize(f32::clamp(yvalue - dv.y, 0.0, 1.0));
                        notifier.notify(ParamChangeEvent {
                            id: y_param,
                            kind: ParamChangeKind::ValueChange(yvalue),
                        });
                    }
                }
                if ui.response().drag_stopped() {
                    let notifier = ui.ctx().plugin_gui_context::<Param>().notifier;
                    notifier.notify(ParamChangeEvent {
                        id: Param::Cutoff,
                        kind: ParamChangeKind::GestureEnd,
                    });
                    notifier.notify(ParamChangeEvent {
                        id: y_param,
                        kind: ParamChangeKind::GestureEnd,
                    });
                }
            })
    }

    fn freq_response(&self, f: f64) -> na::Complex<f64> {
        self.h_z(na::Complex::from_polar(1.0, TAU * f / self.sample_rate))
    }

    fn h_z(&self, z: na::Complex<f64>) -> na::Complex<f64> {
        let [k_in, k_lp, k_bp, k_hp] = self.filter_type.mix_coefficients(self.gain);

        let one = na::Complex::new(1.0, 0.0);
        let one_over_z = z.recip();
        let one_plus_one_over_z = one + one_over_z;
        let one_minus_one_over_z = one - one_over_z;

        let wc = TAU * self.cutoff;
        let r = 2.0 * (1.0 - self.resonance).max(0.0);
        let denominator_common = r * self.sample_rate * wc / one_plus_one_over_z
            - r * self.sample_rate * wc / (z * one_plus_one_over_z)
            + self.sample_rate.powi(2) * one_minus_one_over_z.powi(2) / one_plus_one_over_z.powi(2)
            + wc.powi(2) / 4.0;

        let hp_term = self.sample_rate.powi(2) * k_hp * one_minus_one_over_z.powi(2)
            / (one_plus_one_over_z.powi(2) * denominator_common);
        let bp_term1 = self.sample_rate * k_bp * wc / (2.0 * one_plus_one_over_z * denominator_common);
        let bp_term2 = -self.sample_rate * k_bp * wc / (2.0 * z * one_plus_one_over_z * denominator_common);
        let in_term = k_in;
        let lp_term = k_lp * wc.powi(2) / (4.0 * denominator_common);

        hp_term + bp_term1 + bp_term2 + in_term + lp_term
    }
}

pub(crate) struct SvfMixerGui;

impl EguiPluginView for SvfMixerGui {
    type Params = Param;

    fn update(&mut self, ctx: &Context, _queue: &mut Queue) {
        let style = ctx.style();
        egui::CentralPanel::default()
            .frame(Frame::central_panel(&style).inner_margin(WINDOW_PADDING))
            .show(ctx, |ui| {
                ui.allocate_ui(
                    emath::vec2(WIDTH - 2.0 * WINDOW_PADDING, KNOBS_HEIGHT),
                    generic_ui::display::<Param>,
                );
                SvfFreqResponse::from_params(ui).show("freq_response", ui);
            });
    }
}

pub fn view() -> Result<Box<(dyn PluginView<Params = Param, SharedData = ()> + 'static)>, PluginError> {
    clogbox_clap_egui::view(
        GuiSize {
            width: WIDTH.ceil() as _,
            height: HEIGHT.ceil() as _,
        },
        SvfMixerGui,
    )
}
