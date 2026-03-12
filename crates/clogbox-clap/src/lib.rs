//! # CLAP wrapper for `clogbox`
//!
//! A high-level wrapper around the CLAP audio plugin API, providing simplified
//! abstractions for building audio plugins with the CLAP standard.
//!
//! This crate provides:
//! - Plugin lifecycle management
//! - Parameter handling
//! - Audio processing
//! - State management
//! - GUI integration (when the "gui" feature is enabled)
//!
//! Use the `export_plugin!` macro to easily export your plugin implementation.

#![warn(missing_docs)]
use crate::main_thread::MainThread;
#[cfg(feature = "gui")]
use crate::notifier::Notifier;
use crate::processor::Processor;
use crate::shared::{Shared, SharedData};
use clack_extensions::audio_ports::PluginAudioPorts;
use clack_extensions::note_ports::PluginNotePorts;
use clack_extensions::params::PluginParams;
use clack_extensions::state::PluginState;
pub use clack_plugin::clack_export_entry;
pub use clack_plugin::entry::SinglePluginEntry;
use clack_plugin::host::HostMainThreadHandle;
pub use clack_plugin::host::HostSharedHandle;
pub use clack_plugin;
pub use clack_plugin::plugin;
pub use clack_plugin::plugin::features;
use clack_plugin::prelude::*;
use std::ffi::CStr;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

pub use main_thread::{Layout, Plugin, PluginConfiguration};
pub use processor::{PluginCreateContext, PluginDsp};

use clack_plugin::plugin::Plugin as ClapPlugin;

mod atomic_linked_list;

#[cfg(feature = "gui")]
pub mod gui;
mod main_thread;
mod notifier;
pub mod params;
mod processor;
mod shared;

/// Trait for defining plugin metadata.
///
/// This trait is used to define the basic metadata for a CLAP plugin,
/// including its identifier, name, version, and supported features.
pub trait PluginMeta {
    /// The unique identifier for the plugin.
    const ID: &'static str;
    /// The human-readable name of the plugin.
    const NAME: &'static str;
    /// The version string of the plugin.
    const VERSION: &'static str;
    /// The list of CLAP features supported by the plugin.
    const FEATURES: &'static [&'static CStr];
}

/// A wrapper struct that implements the CLAP plugin interface.
///
/// This struct serves as the entry point for a CLAP plugin implementation,
/// handling the plugin lifecycle, audio processing, and parameter management.
/// It uses the provided plugin type `P` to implement the actual functionality.
pub struct PluginEntry<P: main_thread::Plugin>(PhantomData<P>);

impl<P: main_thread::Plugin> Default for PluginEntry<P> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<P: main_thread::Plugin<Dsp: processor::PluginDsp<Plugin = P>>> ClapPlugin for PluginEntry<P> {
    type AudioProcessor<'a> = Processor<'a, P::Dsp>;
    type Shared<'a> = Shared<P>;
    type MainThread<'a> = MainThread<'a, P>;

    fn declare_extensions(builder: &mut PluginExtensions<Self>, _: Option<&Self::Shared<'_>>) {
        builder
            .register::<PluginAudioPorts>()
            .register::<PluginParams>()
            .register::<PluginState>()
            .register::<PluginNotePorts>();
        #[cfg(feature = "gui")]
        builder.register::<clack_extensions::gui::PluginGui>();
    }
}

impl<P: main_thread::Plugin<Dsp: processor::PluginDsp<Plugin = P>> + PluginMeta> DefaultPluginFactory
    for PluginEntry<P>
{
    fn get_descriptor() -> PluginDescriptor {
        PluginDescriptor::new(P::ID, P::NAME)
            .with_version(P::VERSION)
            .with_features(P::FEATURES.iter().copied())
    }

    fn new_shared(host: HostSharedHandle) -> Result<Self::Shared<'_>, PluginError> {
        Ok(SharedData {
            params: Default::default(),
            #[cfg(feature = "gui")]
            notifier: Notifier::new(),
            user_data: P::shared_data(host)?,
            sample_rate: Arc::new(AtomicU64::new(0)),
        })
    }

    fn new_main_thread<'a>(
        host: HostMainThreadHandle<'a>,
        shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        MainThread::new(host, shared)
    }
}

/// Exports a CLAP plugin implementation.
///
/// This macro simplifies the process of exporting a plugin implementation to be used
/// by CLAP hosts. It takes a plugin type that implements the necessary traits and
/// generates the required entry point code.
///
/// # Example
///
/// ```
/// use std::ffi::CStr;
/// use clack_plugin::prelude::*;
/// # use clogbox_clap::{features, export_plugin, Plugin, PluginMeta, PluginDsp, PluginConfiguration, PluginCreateContext, Layout};
/// # use clogbox_clap::gui::PluginView;
/// use clogbox_enum::Empty;
/// use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
/// use clogbox_module::context::ProcessContext;
///
/// struct Dsp;
///
/// impl Module for Dsp {
///     type Sample = f32;
///     type AudioIn = Empty;
///     type AudioOut = Empty;
///     type ParamsIn = Empty;
///     type ParamsOut = Empty;
///     type NoteIn = Empty;
///     type NoteOut = Empty;
///
///     fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
///         PrepareResult { latency: 0.0 }
///     }
///
///     fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
///         ProcessResult { tail: None }
///     }
/// }
///
/// impl PluginDsp for Dsp {
///     type Plugin = MyPlugin;
///     fn create(_context: PluginCreateContext<Self>, _shared_data: &<Self::Plugin as Plugin>::SharedData) -> Self {
///         Self
///     }
/// }
///
/// struct MyPlugin;
///
/// impl PluginMeta for MyPlugin {
///     const ID: &'static str = "com.myproject.MyPlugin";
///     const NAME: &'static str = "My Plugin";
///     const VERSION: &'static str = env!("CARGO_PKG_VERSION");
///     const FEATURES: &'static [&'static CStr] = &[features::AUDIO_EFFECT, features::STEREO];
/// }
///
/// impl Plugin for MyPlugin {
///     type Dsp = Dsp;
///     type Params = Empty;
///     type SharedData = ();
///     const AUDIO_IN_LAYOUT: &'static [Layout<<Self::Dsp as Module>::AudioIn>] = &[];
///     const AUDIO_OUT_LAYOUT: &'static [Layout<<Self::Dsp as Module>::AudioIn>] = &[];
///
///     fn create(host: HostSharedHandle, configuration: &mut PluginConfiguration) -> Result<Self, PluginError> {
///         todo!()
///     }
///
///     fn shared_data(host: HostSharedHandle) -> Result<Self::SharedData, PluginError> {
///         todo!()
///     }
///
///     fn view(&mut self) -> Result<Box<dyn PluginView<Params=Self::Params, SharedData=Self::SharedData>>, PluginError> {
///         todo!()
///     }
/// }
///
/// export_plugin!(MyPlugin);
/// ```
#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        $crate::clack_export_entry!($crate::SinglePluginEntry<$crate::PluginEntry<$plugin>>);
    };
}
