extern crate core;

use crate::dsp::AudioIn;
use arc_swap::ArcSwap;
use clogbox_clap::gui::PluginView;
use clogbox_clap::main_thread::{Plugin, PortLayout};
use clogbox_clap::processor::{HostSharedHandle, PluginError};
use clogbox_clap::{export_plugin, features, PluginMeta};
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

pub struct Compressor;

impl PluginMeta for Compressor {
    const ID: &'static str = "dev.solarliner.clogbox.compressor";
    const NAME: &'static str = "Compressor";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const FEATURES: &'static [&'static CStr] = &[features::AUDIO_EFFECT, features::STEREO, features::COMPRESSOR];
}

impl Plugin for Compressor {
    type Dsp = dsp::Dsp;
    type Params = dsp::Params;
    type SharedData = SharedData;

    const INPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioIn>] = &[
        PortLayout {
            name: "Input",
            main: true,
            channel_map: &[AudioIn::Input(Stereo::Left), AudioIn::Input(Stereo::Right)],
        },
        PortLayout {
            name: "Sidechain",
            main: false,
            channel_map: &[AudioIn::Sidechain(Stereo::Left), AudioIn::Sidechain(Stereo::Right)],
        },
    ];
    const OUTPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioOut>] = &[PortLayout::STEREO];

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

export_plugin!(Compressor);
