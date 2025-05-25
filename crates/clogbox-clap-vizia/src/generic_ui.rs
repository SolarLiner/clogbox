use crate::components::Knob;
use crate::data::ParamLens;
use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::PluginView;
use clogbox_clap::main_thread::Plugin;
use clogbox_clap::params::{ParamChangeEvent, ParamChangeKind, ParamId};
use clogbox_clap::processor::PluginError;
use clogbox_enum::{enum_iter, Enum};
use vizia::prelude::*;

pub fn generic_view<P: Plugin>(cx: &mut Context) -> Handle<impl View> {
    HStack::new(cx, |cx| {
        for param in enum_iter::<P::Params>() {
            // TODO: Discrete value
            VStack::new(cx, |cx| {
                Knob::<P>::new(cx, param).class("generic-knob");
                Label::new(
                    cx,
                    ParamLens::<P>::new(param).map(move |value| {
                        format!(
                            "{}:\n{}",
                            param.name(),
                            param
                                .value_to_string(value.get())
                                .unwrap_or_else(|_| String::from("<error>"))
                        )
                    }),
                );
            })
            .alignment(Alignment::Center);
        }
    })
    .class("generic-ui")
}

pub fn generic_ui<P: Plugin>(
    size: GuiSize,
) -> Result<Box<dyn PluginView<Params = P::Params, SharedData = P::SharedData>>, PluginError> {
    super::view::<P>(size, |cx| {
        generic_view::<P>(cx);
    })
}
