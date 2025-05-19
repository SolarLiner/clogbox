use crate::dsp;
use crate::dsp::{Params, NUM_STAGES};
use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::PluginView;
use clogbox_clap::params::{ParamChangeEvent, ParamChangeKind, ParamId};
use clogbox_clap::PluginError;
use clogbox_clap_egui::components::driven::Driven;
use clogbox_clap_egui::egui::{emath, Align, Context, Layout};
use clogbox_clap_egui::egui_baseview::Queue;
use clogbox_clap_egui::generic_ui::show_knob;
use clogbox_clap_egui::{components, egui, generic_ui, EguiPluginView, GetContextExtra};

const WINDOW_PADDING: f32 = 10.0;
const LED_WIDTH: f32 = 3.0 * components::led::DEFAULT_RADIUS;
const KNOBS_WIDTH: f32 = 6.0 * generic_ui::KNOB_SIZE;

const WIDTHF: f32 = 2.0 * WINDOW_PADDING + KNOBS_WIDTH + LED_WIDTH;
const WIDTH: u32 = WIDTHF as _;
const HEIGHT: u32 = 250;

struct View;

impl EguiPluginView for View {
    type Params = dsp::Params;

    fn update(&mut self, ctx: &Context, _queue: &mut Queue) {
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ctx.style().as_ref()).inner_margin(WINDOW_PADDING))
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    let height = ui.available_height();
                    let generic_size = emath::vec2(KNOBS_WIDTH, height);
                    ui.allocate_ui(generic_size, |ui| {
                        generic_ui::display_with::<Self::Params>(ui, |ui, rect, param| match param {
                            Params::NumStages => {
                                let gui_context = ui.plugin_gui_context::<Params>();
                                let mut value = gui_context.params[Params::NumStages].get() as u8;
                                log::debug!("rect: {}x{} around {}", rect.width(), rect.height(), rect.center());
                                ui.allocate_ui(rect.size(), |ui| {
                                    let response = ui.add(
                                        egui::DragValue::new(&mut value)
                                            .custom_formatter(|f, _| {
                                                param
                                                    .value_to_string(f as _)
                                                    .unwrap_or_else(|_| String::from("<error>"))
                                            })
                                            .range(1..=NUM_STAGES as _),
                                    );
                                    if response.drag_started() {
                                        gui_context.notifier.notify(ParamChangeEvent {
                                            id: param,
                                            kind: ParamChangeKind::GestureBegin,
                                        });
                                    }
                                    if response.drag_stopped() {
                                        gui_context.notifier.notify(ParamChangeEvent {
                                            id: param,
                                            kind: ParamChangeKind::GestureBegin,
                                        });
                                    }
                                    if response.changed() {
                                        gui_context.params[Params::NumStages].set(value as f32);
                                        gui_context.notifier.notify(ParamChangeEvent {
                                            id: param,
                                            kind: ParamChangeKind::ValueChange(value as f32),
                                        });
                                    }
                                    ui.set_clip_rect(ui.available_rect_before_wrap());
                                });
                            }
                            _ => show_knob(ui, rect.width(), param),
                        });
                    });
                    ui.allocate_ui_with_layout(emath::vec2(LED_WIDTH, height), Layout::top_down(Align::Center), |ui| {
                        for led in ui.plugin_shared_data::<crate::NrClipper>().drive_led.iter().rev() {
                            Driven::by_atomic(led).show(ui, |ui, current| {
                                ui.add(components::Led {
                                    current,
                                    ..Default::default()
                                });
                            });
                        }
                    });
                });
            });
    }
}

pub(crate) fn create() -> Result<Box<dyn PluginView<Params = Params, SharedData = crate::SharedData>>, PluginError> {
    clogbox_clap_egui::view(
        GuiSize {
            width: WIDTH,
            height: HEIGHT,
        },
        View,
    )
}
