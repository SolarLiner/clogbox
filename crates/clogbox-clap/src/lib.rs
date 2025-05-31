use crate::main_thread::{MainThread, Plugin};
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
use clack_plugin::host::{HostMainThreadHandle, HostSharedHandle};
pub use clack_plugin::plugin::features;
use clack_plugin::plugin::{PluginDescriptor, PluginError};
use clack_plugin::prelude::*;
use std::ffi::CStr;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

mod atomic_linked_list;
#[cfg(feature = "gui")]
pub mod gui;
pub mod main_thread;
mod notifier;
pub mod params;
pub mod processor;
pub mod shared;

pub trait PluginMeta {
    const ID: &'static str;
    const NAME: &'static str;
    const VERSION: &'static str;
    const FEATURES: &'static [&'static CStr];
}

pub struct PluginEntry<P: Plugin>(PhantomData<P>);

impl<P: Plugin> clack_plugin::plugin::Plugin for PluginEntry<P> {
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

impl<P: Plugin + PluginMeta> DefaultPluginFactory for PluginEntry<P> {
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

#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        $crate::clack_export_entry!($crate::SinglePluginEntry<$crate::PluginEntry<$plugin>>);
    };
}
