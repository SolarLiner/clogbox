mod dsp;
mod gen;
mod params;

use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::PluginView;
use clogbox_clap::main_thread::{Plugin, PortLayout};
use clogbox_clap::processor::{HostSharedHandle, PluginError};
use clogbox_clap::{export_plugin, features, PluginMeta};
use clogbox_module::Module;
use std::ffi::CStr;

pub struct SvfMixer;

impl PluginMeta for SvfMixer {
    const ID: &'static str = "dev.solarliner.clogbox.SvfMixer";
    const NAME: &'static str = "SVF Mixer";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const FEATURES: &'static [&'static CStr] = &[features::AUDIO_EFFECT, features::STEREO, features::FILTER];
}

impl Plugin for SvfMixer {
    type Dsp = dsp::Dsp;
    type Params = params::Param;
    type SharedData = ();

    const INPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioIn>] =
        &[PortLayout::STEREO.main().named("Input")];
    const OUTPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioOut>] =
        &[PortLayout::STEREO.main().named("Output")];

    fn create(_: HostSharedHandle) -> Result<Self, PluginError> {
        Ok(Self)
    }

    fn shared_data(host: HostSharedHandle) -> Result<Self::SharedData, PluginError> {
        Ok(())
    }

    fn view(&mut self) -> Result<Box<dyn PluginView<Params = Self::Params, SharedData = ()>>, PluginError> {
        clogbox_clap_egui::generic_ui::generic_ui(GuiSize {
            width: 750,
            height: 400,
        })
    }
}

export_plugin!(SvfMixer);
