extern crate core;

use crate::dsp::AudioOut;
use arc_swap::ArcSwap;
use clogbox_clap::gui::PluginView;
use clogbox_clap::{export_plugin, features, PluginMeta};
use clogbox_clap::{HostSharedHandle, PluginError};
use clogbox_clap::{Plugin, PortLayout};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::Stereo;
use clogbox_module::Module;
use clogbox_utils::AtomicF32;
use std::ffi::CStr;
use std::sync::Arc;

mod dsp;
mod gui;

pub struct SharedPluginData {
    pub samplerate: AtomicF32,
    pub cb: ArcSwap<Option<fixed_ringbuf::Consumer<EnumMapArray<Stereo, f32>>>>,
}

pub type SharedData = Arc<SharedPluginData>;

pub struct EnvFollowerPlugin;

impl PluginMeta for EnvFollowerPlugin {
    const ID: &'static str = "dev.solarliner.clogbox.env-follower";
    const NAME: &'static str = "Env Follower";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const FEATURES: &'static [&'static CStr] = &[features::AUDIO_EFFECT, features::STEREO, features::REVERB];
}

impl Plugin for EnvFollowerPlugin {
    type Dsp = dsp::Dsp;
    type Params = dsp::Params;
    type SharedData = SharedData;

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

    fn shared_data(_: HostSharedHandle) -> Result<Self::SharedData, PluginError> {
        Ok(Arc::new(SharedPluginData {
            samplerate: AtomicF32::new(f32::NAN),
            cb: ArcSwap::from_pointee(None),
        }))
    }

    fn view(
        &mut self,
    ) -> Result<Box<dyn PluginView<Params = Self::Params, SharedData = Self::SharedData>>, PluginError> {
        gui::view()
    }
}

export_plugin!(EnvFollowerPlugin);
