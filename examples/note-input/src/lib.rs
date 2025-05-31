use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::PluginView;
use clogbox_clap::main_thread::{Layout, Plugin};
use clogbox_clap::processor::{HostSharedHandle, PluginError};
use clogbox_clap::{features, PluginMeta};
use clogbox_enum::{seq, Empty};
use clogbox_module::Module;
use std::ffi::CStr;

mod dsp;

struct NoteInput;

impl Plugin for NoteInput {
    type Dsp = dsp::Dsp;
    type Params = Empty;
    type SharedData = ();
    const AUDIO_IN_LAYOUT: &'static [Layout<<Self::Dsp as clogbox_module::Module>::AudioIn>] = &[];
    const AUDIO_OUT_LAYOUT: &'static [Layout<<Self::Dsp as clogbox_module::Module>::AudioOut>] = &[Layout::MONO];
    const NOTE_IN_LAYOUT: &'static [Layout<<Self::Dsp as Module>::NoteIn>] = &[Layout {
        name: "Note In",
        main: true,
        channel_map: &[seq(0)],
    }];
    const NOTE_OUT_LAYOUT: &'static [Layout<<Self::Dsp as Module>::NoteOut>] = &[Layout {
        name: "Note Out",
        main: true,
        channel_map: &[seq(0)],
    }];

    fn create(_: HostSharedHandle) -> Result<Self, PluginError> {
        Ok(Self)
    }

    fn shared_data(_: HostSharedHandle) -> Result<Self::SharedData, PluginError> {
        Ok(())
    }

    fn view(
        &mut self,
    ) -> Result<Box<dyn PluginView<Params = Self::Params, SharedData = Self::SharedData>>, PluginError> {
        clogbox_clap_egui::generic_ui(GuiSize {
            width: 500,
            height: 250,
        })
    }
}

impl PluginMeta for NoteInput {
    const ID: &'static str = "dev.solarliner.clogbox.NoteInput";
    const NAME: &'static str = "Note Input";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const FEATURES: &'static [&'static CStr] = &[features::MONO, features::SYNTHESIZER];
}

clogbox_clap::export_plugin!(NoteInput);
