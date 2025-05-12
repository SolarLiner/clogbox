extern crate core;

use crate::dsp::AudioOut;
use arc_swap::ArcSwap;
use clogbox_clap::gui::PluginView;
use clogbox_clap::main_thread::{Plugin, PortLayout};
use clogbox_clap::processor::{HostSharedHandle, PluginError};
use clogbox_clap::{export_plugin, features, PluginMeta};
use clogbox_enum::Stereo;
use clogbox_module::modules::extract::CircularBuffer;
use clogbox_module::Module;
use std::ffi::CStr;
use std::sync::Arc;

mod dsp;
mod gui;

#[derive(Clone)]
pub struct PluginData {
    pub samplerate: f32,
    pub cb: Arc<ArcSwap<Option<CircularBuffer<f32, Stereo>>>>,
}

pub struct EnvFollowerPlugin;

impl PluginMeta for EnvFollowerPlugin {
    const ID: &'static str = "dev.solarliner.clogbox.fundsp-plugin";
    const NAME: &'static str = "FunDSP Plugin";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const FEATURES: &'static [&'static CStr] = &[features::AUDIO_EFFECT, features::STEREO, features::REVERB];
}

impl Plugin for EnvFollowerPlugin {
    type Dsp = dsp::Dsp;
    type Params = dsp::Params;
    type SharedData = PluginData;

    const INPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioIn>] =
        &[PortLayout::STEREO.named("Input").main()];
    const OUTPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioOut>] = &[
        PortLayout {
            main: true,
            name: "Output",
            channel_map: &[AudioOut::Output(Stereo::Left), AudioOut::Output(Stereo::Right)],
        },
        PortLayout {
            main: false,
            name: "Envelope",
            channel_map: &[AudioOut::Envelope(Stereo::Left), AudioOut::Envelope(Stereo::Right)],
        },
    ];

    fn create(_: HostSharedHandle) -> Result<Self, PluginError> {
        Ok(Self)
    }

    fn shared_data(host: HostSharedHandle) -> Result<Self::SharedData, PluginError> {
        Ok(PluginData {
            samplerate: 0.0,
            cb: Arc::new(ArcSwap::from_pointee(None)),
        })
    }

    fn view(
        &mut self,
    ) -> Result<Box<dyn PluginView<Params = Self::Params, SharedData = Self::SharedData>>, PluginError> {
        gui::view()
    }
}

export_plugin!(EnvFollowerPlugin);
