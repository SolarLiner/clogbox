use std::marker::PhantomData;
use crate::params::{ParamId, ParamStorage};
use clack_plugin::prelude::*;
use crate::Plugin;

pub type Shared<P> = SharedData<<P as Plugin>::Params, <P as Plugin>::SharedData>;

#[derive(Debug, Clone)]
pub struct SharedData<Params: ParamId, UserData> {
    pub params: ParamStorage<Params>,
    pub user_data: UserData,
}

impl<Params: ParamId, UserData: 'static + Send + Sync> PluginShared<'_> for SharedData<Params, UserData> {}
