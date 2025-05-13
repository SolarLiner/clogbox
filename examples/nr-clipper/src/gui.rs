use crate::dsp;
use crate::dsp::Params;
use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::PluginView;
use clogbox_clap::processor::PluginError;
use clogbox_clap_egui::components::driven::Driven;
use clogbox_clap_egui::egui::{emath, Context};
use clogbox_clap_egui::egui_baseview::Queue;
use clogbox_clap_egui::{components, egui, generic_ui, EguiPluginView, GetContextExtra};

const WIDTH: u32 = 250;
const WIDTHF: f32 = WIDTH as f32;

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
                Driven::by_atomic(&ui.plugin_shared_data::<crate::NrClipper>().drive_led).show(ui, |ui, current| {
                    ui.add(components::Led {
                        current,
                        ..Default::default()
                    });
                });
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
