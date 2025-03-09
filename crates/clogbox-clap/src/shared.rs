use crate::params::{ParamId, ParamStorage};
use clack_plugin::prelude::*;

#[derive(Debug, Clone)]
pub struct Shared<E: Send + ParamId> {
    pub params: ParamStorage<E>,
}

impl<E: Send + ParamId> Default for Shared<E> {
    fn default() -> Self {
        Self {
            params: ParamStorage::default(),
        }
    }
}

impl<E: Sync + ParamId> PluginShared<'_> for Shared<E> {}
