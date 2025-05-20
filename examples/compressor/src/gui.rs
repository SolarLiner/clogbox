use std::time::Duration;

use crate::{dsp, SharedData};
use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::PluginView;
use clogbox_clap::processor::PluginError;
use clogbox_clap_egui::egui::{Context, Layout, Vec2};
use clogbox_clap_egui::egui_baseview::Queue;
use clogbox_clap_egui::{egui, generic_ui, shared_data_id, EguiPluginView};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{enum_iter, Enum, Stereo};
use clogbox_math::linear_to_db;

pub struct View {
    data: Vec<EnumMapArray<Stereo, f32>>,
}

impl EguiPluginView for View {
    type Params = dsp::Params;

    fn update(&mut self, ctx: &Context, _queue: &mut Queue) {
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ctx.style().as_ref()).inner_margin(10.0))
            .show(ctx, |ui| {
                ui.with_layout(Layout::left_to_right(egui::Align::Min), |ui| {
                    ui.allocate_ui(Vec2::new(400.0, 300.0), |ui| {
                        generic_ui::display::<dsp::Params>(ui);
                    });
                    let shared_data: SharedData = ui.ctx().data(|data| data.get_temp(shared_data_id()).unwrap());
                    let cb = shared_data.cb.load();
                    let Some(rx) = cb.as_ref() else { return };
                    self.data.clear();
                    self.data.extend(rx.iter().cloned());
                    let len = self.data.len();
                    let samplerate = shared_data.samplerate.load(std::sync::atomic::Ordering::Relaxed) as f64;
                    ui.vertical(|ui| {
                        egui_plot::Plot::new("envelope")
                            .width(300.0)
                            .height(500.0)
                            .allow_zoom([false, false])
                            .allow_boxed_zoom(false)
                            .allow_drag([false, false])
                            .allow_scroll([false, false])
                            .auto_bounds([true, true])
                            .default_x_bounds(-1.0, 0.0)
                            .default_y_bounds(-40.0, 6.0)
                            .legend(egui_plot::Legend::default().position(egui_plot::Corner::LeftBottom))
                            .show(ui, |ui| {
                                for e in enum_iter::<Stereo>() {
                                    let plot_points = egui_plot::PlotPoints::from_iter(
                                        self.data.iter().enumerate().map(|(i, frame)| {
                                            let x = (len - i) as f64 / samplerate;
                                            let y = linear_to_db(frame[e]) as f64;
                                            [-x, y]
                                        }),
                                    );
                                    ui.line(egui_plot::Line::new(e.name(), plot_points));
                                }
                                ui.ctx().request_repaint_after(Duration::from_nanos(16_666_667));
                            });
                    });
                })
            });
    }
}

pub fn view() -> Result<Box<dyn PluginView<Params = dsp::Params, SharedData = SharedData>>, PluginError> {
    clogbox_clap_egui::view(
        GuiSize {
            width: 700,
            height: 500,
        },
        View { data: vec![] },
    )
}
