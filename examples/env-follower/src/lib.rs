extern crate core;

use crate::dsp::AudioOut;
use arc_swap::ArcSwap;
use clogbox_clap::gui::PluginView;
use clogbox_clap::main_thread::{Plugin, PortLayout};
use clogbox_clap::processor::{HostSharedHandle, PluginError};
use clogbox_clap::{export_plugin, features, PluginMeta};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::Stereo;
use clogbox_module::Module;
use std::ffi::CStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

mod dsp;
mod gui;

pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub fn new(value: f32) -> Self {
        Self(AtomicU32::new(value.to_bits()))
    }

    pub fn load(&self, order: Ordering) -> f32 {
        f32::from_bits(self.0.load(order))
    }

    pub fn store(&self, value: f32, order: Ordering) {
        self.0.store(value.to_bits(), order);
    }
}

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
