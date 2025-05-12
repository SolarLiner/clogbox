use clogbox_clap::main_thread::{Plugin, PortLayout};
use clogbox_clap::processor::{HostSharedHandle, PluginError};
use clogbox_clap::{export_plugin, PluginMeta};
use clogbox_module::Module;

mod dsp;
mod gen;

pub struct NrClipper;

impl PluginMeta for NrClipper {
    const ID: &'static str = "dev.solarliner.clogbox.nr-clipper";
    const NAME: &'static str = "NR Clipper";
}

impl Plugin for NrClipper {
    type Dsp = dsp::Dsp;
    type Params = dsp::Params;
    type SharedData = ();

    const INPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioIn>] =
        &[PortLayout::STEREO.main().named("Input")];
    const OUTPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioOut>] =
        &[PortLayout::STEREO.main().named("Output")];

    fn create(_: HostSharedHandle) -> Result<Self, PluginError> {
        Ok(Self)
    }

    fn shared_data(_: HostSharedHandle) -> Result<Self::SharedData, PluginError> {
        Ok(())
    }
}

export_plugin!(NrClipper);
