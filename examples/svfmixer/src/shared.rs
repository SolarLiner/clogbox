use clack_plugin::prelude::*;
use crate::params;

#[derive(Debug, Clone, Default)]
pub struct SvfMixerShared {
    pub(crate) params: params::Storage,
}

impl PluginShared<'_> for SvfMixerShared {}