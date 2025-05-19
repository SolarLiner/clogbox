//! Implementation of the audio thread side of a CLAP plugin.
use crate::params::ParamId;
use crate::Plugin;
pub use clack_plugin::host::HostSharedHandle;
pub use clack_plugin::prelude::PluginAudioConfiguration;
use clogbox_enum::enum_map::EnumMapRef;
use clogbox_enum::Empty;
use clogbox_module::Module;

/// Context at plugin creation
pub struct PluginCreateContext<'a, 'p, P: ?Sized + PluginDsp> {
    /// CLAP host interface
    pub host: HostSharedHandle<'a>,
    /// Plugin entry point
    pub plugin_entry_point: &'p mut P::Plugin,
    /// Reference to the current parameter values
    pub params: EnumMapRef<'p, P::ParamsIn, f32>,
    /// CLAP audio configuration
    pub audio_config: PluginAudioConfiguration,
}

/// A DSP module that can also be used as the audio processor for a plugin.
///
/// TODO: Note support
pub trait PluginDsp:
    Send + Module<Sample = f32, ParamsIn: ParamId, ParamsOut = Empty, NoteIn = Empty, NoteOut = Empty>
{
    /// Type of the main plugin
    type Plugin: Plugin<Dsp = Self, Params = Self::ParamsIn>;

    /// Instantiate this type to begin audio processing.
    fn create(context: PluginCreateContext<Self>, shared_data: &<Self::Plugin as Plugin>::SharedData) -> Self;
}
