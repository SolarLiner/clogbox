use clogbox_clap::gui::PluginView;
use clogbox_clap::main_thread::{Plugin, PortLayout};
use clogbox_clap::processor::{HostSharedHandle, PluginError};
use clogbox_clap::{export_plugin, features, PluginMeta};
use clogbox_module::Module;
use clogbox_utils::AtomicF32;
use std::ffi::CStr;
use std::sync::Arc;

mod dsp;
mod gen;
mod gui;

pub struct SharedDataInner {
    drive_led: [AtomicF32; dsp::NUM_STAGES],
}

pub type SharedData = Arc<SharedDataInner>;

pub struct NrClipper;

impl PluginMeta for NrClipper {
    const ID: &'static str = "dev.solarliner.clogbox.nr-clipper";
    const NAME: &'static str = "NR Clipper";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const FEATURES: &'static [&'static CStr] = &[
        features::STEREO,
        features::AUDIO_EFFECT,
        features::DISTORTION,
        features::FILTER,
    ];
}

impl Plugin for NrClipper {
    type Dsp = dsp::Dsp;
    type Params = dsp::Params;
    type SharedData = SharedData;

    const INPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioIn>] =
        &[PortLayout::STEREO.main().named("Input")];
    const OUTPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioOut>] =
        &[PortLayout::STEREO.main().named("Output")];

    fn create(_: HostSharedHandle) -> Result<Self, PluginError> {
        Ok(Self)
    }

    fn shared_data(_: HostSharedHandle) -> Result<Self::SharedData, PluginError> {
        Ok(SharedData::new(SharedDataInner {
            drive_led: std::array::from_fn(|_| AtomicF32::new(0.0)),
        }))
    }

    fn view(
        &mut self,
    ) -> Result<Box<dyn PluginView<Params = Self::Params, SharedData = Self::SharedData>>, PluginError> {
        gui::create()
    }
}

export_plugin!(NrClipper);
