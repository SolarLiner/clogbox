use clogbox_clap::main_thread::{Plugin, PortLayout};
use clogbox_clap::processor::{HostSharedHandle, PluginError};
use clogbox_clap::{export_plugin, features, PluginMeta};
use clogbox_enum::{seq, Sequential};
use clogbox_module::Module;
use fundsp::prelude::U2;
use std::ffi::CStr;

mod dsp;

pub struct FundspPlugin;

const fn port_layout() -> PortLayout<Sequential<U2>> {
    const CHANNEL_MAP: [Sequential<U2>; 2] = [seq(0), seq(1)];
    PortLayout {
        main: true,
        name: "Stereo",
        channel_map: &CHANNEL_MAP,
    }
}

impl PluginMeta for FundspPlugin {
    const ID: &'static str = "dev.solarliner.clogbox.fundsp-plugin";
    const NAME: &'static str = "FunDSP Plugin";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const FEATURES: &'static [&'static CStr] = &[features::AUDIO_EFFECT, features::STEREO, features::REVERB];
}

impl Plugin for FundspPlugin {
    type Dsp = dsp::Dsp;
    type Params = dsp::Params;
    const INPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioIn>] = &[port_layout()];
    const OUTPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioOut>] = &[port_layout()];

    fn create(_: HostSharedHandle) -> Result<Self, PluginError> {
        Ok(Self)
    }
}

export_plugin!(FundspPlugin);
