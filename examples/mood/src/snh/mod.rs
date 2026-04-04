use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::PluginView;
use clogbox_clap::plugin::PluginError;
use clogbox_clap::HostSharedHandle;
use clogbox_clap::{features, Layout, Plugin, PluginConfiguration, PluginMeta};
use clogbox_module::Module;
use std::ffi::CStr;

mod dsp;

pub struct MoodBBD;

impl PluginMeta for MoodBBD {
    const ID: &'static str = "dev.solarliner.clogbox.mood-bbd";
    const NAME: &'static str = "M0od BBD";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const FEATURES: &'static [&'static CStr] = &[
        features::STEREO,
        features::AUDIO_EFFECT,
        features::DISTORTION,
        features::FILTER,
    ];
}

impl Plugin for MoodBBD {
    type Dsp = dsp::Dsp;
    type Params = dsp::Params;
    type SharedData = ();

    const AUDIO_IN_LAYOUT: &'static [Layout<<Self::Dsp as Module>::AudioIn>] = &[Layout::STEREO.main().named("Input")];
    const AUDIO_OUT_LAYOUT: &'static [Layout<<Self::Dsp as Module>::AudioOut>] =
        &[Layout::STEREO.main().named("Output")];

    fn create(_: HostSharedHandle, _: &mut PluginConfiguration) -> Result<Self, PluginError> {
        Ok(Self)
    }

    fn shared_data(_: HostSharedHandle) -> Result<Self::SharedData, PluginError> {
        Ok(())
    }

    fn view(
        &mut self,
    ) -> Result<Box<dyn PluginView<Params = Self::Params, SharedData = Self::SharedData>>, PluginError> {
        clogbox_clap_egui::generic_ui(GuiSize {
            width: 400,
            height: 300,
        })
    }
}
