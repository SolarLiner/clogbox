mod dsp;
mod gen;
mod gui;
mod params;

use clogbox_clap::gui::PluginView;
use clogbox_clap::main_thread::{Layout, Plugin};
use clogbox_clap::processor::{HostSharedHandle, PluginError};
use clogbox_clap::{export_plugin, features, PluginMeta};
use clogbox_module::Module;
use std::ffi::CStr;

pub struct SvfMixer;

impl PluginMeta for SvfMixer {
    const ID: &'static str = "dev.solarliner.clogbox.SvfMixer";
    const NAME: &'static str = "SVF Mixer";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const FEATURES: &'static [&'static CStr] = &[features::STEREO, features::AUDIO_EFFECT, features::FILTER];
}

impl Plugin for SvfMixer {
    type Dsp = dsp::Dsp;
    type Params = params::Param;
    type SharedData = ();

    const AUDIO_IN_LAYOUT: &'static [Layout<<Self::Dsp as Module>::AudioIn>] = &[Layout::STEREO.main().named("Input")];
    const AUDIO_OUT_LAYOUT: &'static [Layout<<Self::Dsp as Module>::AudioOut>] =
        &[Layout::STEREO.main().named("Output")];

    fn create(_: HostSharedHandle) -> Result<Self, PluginError> {
        Ok(Self)
    }

    fn shared_data(_: HostSharedHandle) -> Result<Self::SharedData, PluginError> {
        Ok(())
    }

    fn view(&mut self) -> Result<Box<dyn PluginView<Params = Self::Params, SharedData = ()>>, PluginError> {
        gui::view()
    }
}

export_plugin!(SvfMixer);
