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
use crate::notifier::Notifier;
use crate::params::ParamId;
use crate::processor::Processor;
use crate::shared::{Shared, SharedData};
use clack_extensions::audio_ports::PluginAudioPorts;
use clack_extensions::params::PluginParams;
use clack_extensions::state::PluginState;
pub use clack_plugin::clack_export_entry;
pub use clack_plugin::entry::SinglePluginEntry;
use clack_plugin::host::HostMainThreadHandle;
pub use clack_plugin::host::HostSharedHandle;
pub use clack_plugin::plugin::features;
use clack_plugin::plugin::PluginDescriptor;
pub use clack_plugin::plugin::PluginError;
use clack_plugin::prelude::*;
use clogbox_enum::{Mono, Stereo};
use clogbox_module::Module;
use dsp::PluginDsp;
use std::ffi::CStr;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

mod atomic_linked_list;

pub mod dsp;
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
pub struct PluginEntry<P: Plugin>(PhantomData<P>);

impl<P: Plugin> clack_plugin::plugin::Plugin for PluginEntry<P> {
    type AudioProcessor<'a> = Processor<'a, P::Dsp>;
    type Shared<'a> = Shared<P>;
    type MainThread<'a> = MainThread<'a, P>;

    fn declare_extensions(builder: &mut PluginExtensions<Self>, _: Option<&Self::Shared<'_>>) {
        builder
            .register::<PluginAudioPorts>()
            .register::<PluginParams>()
            .register::<PluginState>();
        #[cfg(feature = "gui")]
        builder.register::<clack_extensions::gui::PluginGui>();
    }
}

impl<P: Plugin + PluginMeta> DefaultPluginFactory for PluginEntry<P> {
    fn get_descriptor() -> PluginDescriptor {
        PluginDescriptor::new(P::ID, P::NAME)
            .with_version(P::VERSION)
            .with_features(P::FEATURES.iter().copied())
    }

    fn new_shared(host: HostSharedHandle) -> Result<Self::Shared<'_>, PluginError> {
        Ok(SharedData {
            params: Default::default(),
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
/// use clogbox_clap::{PluginMeta, Plugin, export_plugin, features};
/// use clogbox_clap::gui::PluginView;
/// use clogbox_clap::PortLayout;
/// use clogbox_module::Module;
///
/// struct MyPlugin;
///
/// impl PluginMeta for MyPlugin {
///     const ID: &'static str = "com.myproject.MyPlugin";
///     const NAME: &'static str = "My Plugin";
///     const VERSION: &'static str = env!("CARGO_PKG_VERSION");
///     const FEATURES: &'static [&'static CStr] = &[features::STEREO];
/// }
///
/// impl Plugin for MyPlugin {
///     type Dsp = ();
///     type Params = ();
///     type SharedData = ();
///     const INPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioIn>] = &[];
///     const OUTPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioOut>] = &[];
///
///     fn create(host: HostSharedHandle) -> Result<Self, PluginError> {
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

/// An audio port in CLAP is a multichannel input or output. CLAP can have multiple input and output ports, but as
/// `clogbox` only works with a flat multichannel layout, it becomes necessary to specify which ports will route to
/// which channels.
///
/// # Example
///
/// ```
/// use clogbox_clap::PortLayout;
/// use clogbox_enum::Enum;
///
/// #[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Enum)]
/// enum AudioInput { MainLeft, MainRight, SidechainMono }
///
/// const MAIN_PORT: PortLayout<AudioInput> = PortLayout::new(&[AudioInput::MainLeft, AudioInput::MainRight])
///     .named("Input")
///     .main();
/// const SIDECHAIN_PORT: PortLayout<AudioInput> = PortLayout::new(&[AudioInput::SidechainMono])
///     .named("Sidechain");
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PortLayout<E: 'static> {
    /// Port name
    pub name: &'static str,
    /// Is this the main port?
    pub main: bool,
    /// Mapping of channels in this port to channels in the input
    pub channel_map: &'static [E],
}

impl<E: 'static> PortLayout<E> {
    /// Create a new [`PortLayout`](Self) with the provided channel map.
    pub const fn new(channel_map: &'static [E]) -> Self {
        Self {
            name: "Input",
            main: false,
            channel_map: &channel_map,
        }
    }

    /// Sets this port layout as the main one.
    pub const fn main(self) -> Self {
        Self { main: true, ..self }
    }

    /// Rename this port layout.
    pub const fn named(self, name: &'static str) -> Self {
        Self { name, ..self }
    }
}

impl PortLayout<Mono> {
    pub const MONO: Self = Self {
        name: "Mono",
        main: false,
        channel_map: &[Mono],
    };
}

impl PortLayout<Stereo> {
    pub const STEREO: Self = Self {
        name: "Stereo",
        main: false,
        channel_map: &[Stereo::Left, Stereo::Right],
    };
}

/// Main plugin trait. This should be implemented by a separate, often empty struct that will serve as the entry
/// point to the plugin. This type will only be useful at compile time and to hold the required functions to
/// construct the audio processor and GUI (if supported).
pub trait Plugin: 'static + Sized {
    /// DSP trait implementing the audio processor
    type Dsp: PluginDsp<Plugin = Self, ParamsIn = Self::Params>;
    /// Parameters of the plugin
    type Params: ParamId;
    /// Shared data between the DSP and GUI
    type SharedData: 'static + Clone + Send + Sync;

    /// Input port map. See [`PortLayout`] for details.
    const INPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioIn>];
    /// Output port map. See [`PortLayout`] for details.
    const OUTPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioOut>];

    /// Create a new plugin instance.
    ///
    /// Don't do work in this method, as this might be instantiated when scanning the plugin.
    ///
    /// # Arguments
    ///
    /// * `host`: CLAP host interface.
    fn create(host: HostSharedHandle) -> Result<Self, PluginError>;

    /// Return this plugin's shared data. This will be made available to the DSP and GUI implementations. Use this to
    /// provide DSP <-> GUI communication, for example.
    ///
    /// # Arguments
    ///
    /// * `host`: CLAP host interface.
    fn shared_data(host: HostSharedHandle) -> Result<Self::SharedData, PluginError>;

    /// Create this plugin's GUI view. Usually, you will use a GUI framework's function to create the
    /// [`gui::PluginView`] for you.
    #[cfg(feature = "gui")]
    fn view(
        &mut self,
    ) -> Result<Box<dyn gui::PluginView<Params = Self::Params, SharedData = Self::SharedData>>, PluginError>;
}
