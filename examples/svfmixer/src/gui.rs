use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::{GuiContext, PluginView};
use clogbox_clap::params::{ParamChangeKind, ParamId};
use clogbox_clap::processor::PluginError;
use clogbox_clap_egui::egui::widgets;
use clogbox_clap_egui::{egui, egui_baseview, EguiPluginView};
use clogbox_enum::{enum_iter, Enum};

struct SvfMixerGui;

impl EguiPluginView for SvfMixerGui {
    type Params = crate::params::Param;

    fn update(
        &mut self,
        ctx: &egui::Context,
        _queue: &mut egui_baseview::Queue,
        gui_context: &GuiContext<Self::Params>,
    ) {
        let params = gui_context.params.clone();
        let dsp_notifier = gui_context.dsp_notifier.clone();
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Grid::new("params")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    for p in enum_iter::<crate::params::Param>() {
                        let mut value = params.get_normalized(p);
                        ui.label(p.name());
                        let widget = widgets::DragValue::new(&mut value)
                            .custom_parser(|s| p.text_to_value(s).map(|f| p.mapping().normalize(f) as _))
                            .custom_formatter(|f, _| {
                                p.value_to_string(p.mapping().denormalize(f as _))
                                    .unwrap_or_else(|err| format!("Formatting error: {err}"))
                            })
                            .speed(0.005)
                            .range(0.0..=1.0);
                        let response = ui.add(widget);
                        ui.end_row();
                        if response.changed() {
                            params.set(p, p.mapping().denormalize(value as _));
                            dsp_notifier.notify(p, ParamChangeKind::ValueChange(p.mapping().denormalize(value as _)));
                        }
                    }
                });
        });
    }
}

pub fn view() -> Result<Box<dyn PluginView<Params = crate::params::Param>>, PluginError> {
    clogbox_clap_egui::view(
        GuiSize {
            width: 300,
            height: 300,
        },
        SvfMixerGui,
    )
}
