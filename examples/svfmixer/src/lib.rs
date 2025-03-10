mod dsp;
mod params;

use clogbox_clap::main_thread::{Plugin, PortLayout};
use clogbox_clap::processor::{HostSharedHandle, PluginDsp, PluginError};
use clogbox_clap::{export_plugin, features, PluginMeta};
use std::ffi::CStr;

struct SvfMixer;

impl PluginMeta for SvfMixer {
    const ID: &'static str = "dev.solarliner.clogbox.SvfMixer";
    const NAME: &'static str = "SVF Mixer";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const FEATURES: &'static [&'static CStr] = &[features::AUDIO_EFFECT, features::STEREO, features::FILTER];
}

impl Plugin for SvfMixer {
    type Dsp = dsp::Dsp;
    type Params = params::Param;

    const INPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as PluginDsp>::Inputs>] =
        &[PortLayout::STEREO.main().named("Input")];
    const OUTPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as PluginDsp>::Outputs>] =
        &[PortLayout::STEREO.main().named("Output")];

    fn create(_: HostSharedHandle) -> Result<Self, PluginError> {
        Ok(Self)
    }
}

export_plugin!(SvfMixer);
