use crate::notifier::Notifier;
use crate::params::{ParamChangeEvent, ParamId, ParamStorage};
use crate::Plugin;
use clack_plugin::prelude::*;

pub type Shared<P> = SharedData<<P as Plugin>::Params, <P as Plugin>::SharedData>;

#[derive(Clone)]
pub struct SharedData<Params: ParamId, UserData> {
    pub params: ParamStorage<Params>,
    pub notifier: Notifier<ParamChangeEvent<Params>>,
    pub user_data: UserData,
}

impl<Params: ParamId, UserData: 'static + Send + Sync> PluginShared<'_> for SharedData<Params, UserData> {}
