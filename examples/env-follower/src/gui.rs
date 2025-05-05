use crate::{dsp, PluginData};
use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::PluginView;
use clogbox_clap::processor::PluginError;
use clogbox_clap_egui::egui::{Context, Pos2, Rect, UiBuilder, Vec2};
use clogbox_clap_egui::egui_baseview::Queue;
use clogbox_clap_egui::{egui, generic_ui, shared_data_id, EguiPluginView};
use clogbox_enum::{enum_iter, Enum, Stereo};
use clogbox_math::linear_to_db;

pub struct View;

impl EguiPluginView for View {
    type Params = dsp::Params;

    fn update(&mut self, ctx: &Context, _queue: &mut Queue) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.allocate_new_ui(
                    UiBuilder::new().max_rect(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
                    |ui| {
                        generic_ui::display::<dsp::Params>(ui);
                    },
                );
                egui_plot::Plot::new("envelope")
                    .width(300.0)
                    .height(300.0)
                    .allow_zoom([false, false])
                    .allow_boxed_zoom(false)
                    .allow_drag([false, false])
                    .allow_scroll([false, false])
                    .auto_bounds([true, false])
                    .default_y_bounds(-40.0, 6.0)
                    .show(ui, |ui| {
                        let shared_data: PluginData = ui.ctx().data(|data| data.get_temp(shared_data_id()).unwrap());
                        let cb = shared_data.cb.load();
                        let Some(cb) = cb.as_ref() else { return };
                        for e in enum_iter::<Stereo>() {
                            ui.line(egui_plot::Line::new(
                                e.name(),
                                cb.iter_frames()
                                    .enumerate()
                                    .map(|(i, frame)| {
                                        let x = i as f32 / shared_data.samplerate;
                                        let y = linear_to_db(frame[e]);
                                        [x as f64, y as f64]
                                    })
                                    .collect::<Vec<_>>(),
                            ))
                        }
                    });
            })
        });
    }
}

pub fn view() -> Result<Box<dyn PluginView<Params = dsp::Params, SharedData = PluginData>>, PluginError> {
    clogbox_clap_egui::view(
        GuiSize {
            width: 700,
            height: 300,
        },
        View,
    )
}
