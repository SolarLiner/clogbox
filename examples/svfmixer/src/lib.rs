mod main_thread;
mod params;
mod processor;
mod shared;

use clack_extensions::audio_ports::PluginAudioPorts;
use clack_extensions::params::PluginParams;
use clack_extensions::state::PluginState;
use clack_plugin::plugin::features::{AUDIO_EFFECT, FILTER, STEREO};
use clack_plugin::prelude::*;

struct SvfMixer;

impl DefaultPluginFactory for SvfMixer {
    fn get_descriptor() -> PluginDescriptor {
        PluginDescriptor::new("dev.solarliner.clogbox.SvfMixer", "SVF Mixer")
            .with_version(env!("CARGO_PKG_VERSION"))
            .with_features([AUDIO_EFFECT, STEREO, FILTER])
    }

    fn new_shared(host: HostSharedHandle) -> Result<Self::Shared<'_>, PluginError> {
        Ok(shared::SvfMixerShared::default())
    }

    fn new_main_thread<'a>(
        _: HostMainThreadHandle<'a>,
        shared: &'a Self::Shared<'a>,
    ) -> Result<Self::MainThread<'a>, PluginError> {
        Ok(main_thread::SvfMixerMainThread {
            shared: shared.clone(),
        })
    }
}

impl Plugin for SvfMixer {
    type AudioProcessor<'a> = processor::SvfMixerProcessor;
    type Shared<'a> = shared::SvfMixerShared;
    type MainThread<'a> = main_thread::SvfMixerMainThread;

    fn declare_extensions(builder: &mut PluginExtensions<Self>, shared: Option<&Self::Shared<'_>>) {
        builder
            .register::<PluginAudioPorts>()
            .register::<PluginParams>()
            .register::<PluginState>();
    }
}

clack_export_entry!(SinglePluginEntry<SvfMixer>);
