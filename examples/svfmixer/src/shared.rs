use crate::params;
use clack_plugin::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct SvfMixerShared {
    pub(crate) params: params::Storage,
}

impl PluginShared<'_> for SvfMixerShared {}
