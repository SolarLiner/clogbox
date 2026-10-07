use clogbox_clap::clack_plugin::entry::prelude::PluginFactoryWrapper;
use clogbox_clap::clack_plugin::entry::{DefaultPluginFactory, Entry, EntryFactories, EntryLoadError};
use clogbox_clap::clack_plugin::factory::plugin::PluginFactoryImpl;
use clogbox_clap::clack_plugin::host::HostInfo;
use clogbox_clap::plugin::{PluginDescriptor, PluginInstance};
use clogbox_clap::{clack_export_entry, Plugin};
use clogbox_clap::{PluginEntry, PluginMeta};
use std::ffi::CStr;

mod chorus;
mod snh;

struct Factory {
    bbd: PluginDescriptor,
    chorus: PluginDescriptor,
}

impl Default for Factory {
    fn default() -> Self {
        Self {
            bbd: PluginEntry::<snh::MoodBBD>::get_descriptor(),
            chorus: PluginEntry::<chorus::MoodChorus>::get_descriptor(),
        }
    }
}

impl PluginFactoryImpl for Factory {
    fn plugin_count(&self) -> u32 {
        2
    }

    fn plugin_descriptor(&self, index: u32) -> Option<&PluginDescriptor> {
        match index {
            0 => Some(&self.bbd),
            1 => Some(&self.chorus),
            _ => None,
        }
    }

    fn create_plugin<'a>(&'a self, host_info: HostInfo<'a>, plugin_id: &CStr) -> Option<PluginInstance<'a>> {
        if plugin_id == self.bbd.id().unwrap() {
            Some(PluginInstance::new::<PluginEntry<snh::MoodBBD>>(
                host_info,
                &self.bbd,
                PluginEntry::<snh::MoodBBD>::new_shared,
                PluginEntry::<snh::MoodBBD>::new_main_thread,
            ))
        } else if plugin_id == self.chorus.id().unwrap() {
            Some(PluginInstance::new::<PluginEntry<chorus::MoodChorus>>(
                host_info,
                &self.chorus,
                PluginEntry::<chorus::MoodChorus>::new_shared,
                PluginEntry::<chorus::MoodChorus>::new_main_thread,
            ))
        } else {
            None
        }
    }
}

struct MoodEntry(PluginFactoryWrapper<Factory>);

impl Entry for MoodEntry {
    fn new(_: &CStr) -> Result<Self, EntryLoadError> {
        Ok(Self(PluginFactoryWrapper::new(Factory::default())))
    }

    fn declare_factories<'a>(&'a self, builder: &mut EntryFactories<'a>) {
        builder.register_factory(&self.0);
    }
}

clack_export_entry!(MoodEntry);
