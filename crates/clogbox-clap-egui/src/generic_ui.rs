use crate::components::Knob;
use crate::{gui_context_id, EguiPluginView};
use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::{GuiContext, PluginView};
use clogbox_clap::params::ParamId;
use clogbox_clap::processor::PluginError;
use clogbox_enum::enum_iter;
use egui::{Align, Layout, Ui};
use std::marker::PhantomData;

pub fn generic_ui<E: ParamId, SharedData: 'static + Send + Sync + Clone>(
    size: GuiSize,
) -> Result<Box<dyn PluginView<Params = E, SharedData = SharedData>>, PluginError> {
    crate::view(size, GenericUi(PhantomData))
}

struct GenericUi<E: ParamId>(PhantomData<E>);

impl<E: ParamId> EguiPluginView for GenericUi<E> {
    type Params = E;

    fn update(&mut self, ctx: &egui::Context, _queue: &mut egui_baseview::Queue) {
        egui::CentralPanel::default().show(ctx, |ui| {
            display::<E>(ui);
        });
    }
}

pub fn display<E: ParamId>(ui: &mut Ui) {
    const KNOB_SIZE: f32 = 60.0;
    let element_width = 2.0 * KNOB_SIZE;
    let gui_context: GuiContext<E> = ui.ctx().data(|data| data.get_temp(gui_context_id()).unwrap());

    ui.style_mut().spacing.item_spacing.y = 15.0;
    egui::ScrollArea::new([false, true]).show(ui, |ui| {
        let rect = ui.max_rect();
        let num_columns = (rect.width() / element_width).floor() as usize;
        egui::Grid::new("knobs").num_columns(num_columns).show(ui, |ui| {
            for (i, param) in enum_iter::<E>().enumerate() {
                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                    ui.set_min_width(element_width);
                    ui.add(Knob::new(ui.ctx(), param).with_knob_size(KNOB_SIZE));
                    let value = gui_context.params[param].get();
                    ui.label(format!(
                        "{}: {}",
                        param.name(),
                        param.value_to_string(value).unwrap_or_else(|_| String::from("<error>"))
                    ))
                });
                if i % num_columns == num_columns - 1 {
                    ui.end_row();
                }
            }
        });
    });
}
