use crate::dsp;
use crate::dsp::Params;
use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::PluginView;
use clogbox_clap::processor::PluginError;
use clogbox_clap_egui::egui::{emath, Context, Response, Ui};
use clogbox_clap_egui::egui_baseview::Queue;
use clogbox_clap_egui::{egui, generic_ui, shared_data_id, EguiPluginView};
use std::sync::atomic::Ordering;
use std::time::Duration;

const WIDTH: u32 = 250;
const WIDTHF: f32 = WIDTH as f32;

struct Led;

fn led_color(current: f32) -> egui::Color32 {
    let r = 1.0 - (-current / 3.0).exp();
    let g = 1.0 - (-current / 16.0).exp();
    let b = g;
    let [r, g, b] = [r, g, b].map(|x| x * 255.0).map(|f| f.round() as u8);
    egui::Color32::from_rgb(r, g, b)
}

impl egui::Widget for Led {
    fn ui(self, ui: &mut Ui) -> Response {
        let (response, painter) = ui.allocate_painter(emath::vec2(10.0, 10.0), egui::Sense::empty());
        let shared: crate::SharedData = ui.ctx().data(|data| data.get_temp(shared_data_id())).unwrap();
        let color = led_color(shared.drive_led.load(Ordering::Relaxed));
        let center = response.rect.center();
        let radius = response.rect.size().min_elem() / 2.0;
        painter.circle_filled(center, radius, color);
        ui.ctx().request_repaint_after(Duration::from_nanos(16_666_667));
        response
    }
}

struct View;

impl EguiPluginView for View {
    type Params = dsp::Params;

    fn update(&mut self, ctx: &Context, _queue: &mut Queue) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                let generic_width = WIDTHF - 50.0;
                let height = ui.available_height();
                let generic_size = emath::vec2(generic_width, height);
                ui.allocate_ui(generic_size, |ui| {
                    generic_ui::display::<Self::Params>(ui);
                });
                ui.add(Led);
            });
        });
    }
}

pub(crate) fn create() -> Result<Box<dyn PluginView<Params = Params, SharedData = crate::SharedData>>, PluginError> {
    clogbox_clap_egui::view(
        GuiSize {
            width: WIDTH,
            height: 150,
        },
        View,
    )
}
