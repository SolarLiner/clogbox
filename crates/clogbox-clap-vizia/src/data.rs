use clogbox_clap::main_thread::Plugin;
use clogbox_clap::params::ParamValue;
use clogbox_enum::Enum;
use std::fmt;
use std::marker::PhantomData;
use vizia::prelude::*;

pub struct ParamLens<P: Plugin> {
    __plugin: PhantomData<P>,
    param: P::Params,
}

impl<P: Plugin> fmt::Debug for ParamLens<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ParamLens").field(&self.param.name()).finish()
    }
}

impl<P: Plugin> Clone for ParamLens<P> {
    fn clone(&self) -> Self {
        Self {
            __plugin: PhantomData,
            param: self.param.clone(),
        }
    }
}

impl<P: Plugin> Copy for ParamLens<P> {}

impl<P: Plugin> ParamLens<P> {
    pub fn new(param: P::Params) -> Self {
        Self {
            __plugin: PhantomData,
            param,
        }
    }
}

impl<P: Plugin> Lens for ParamLens<P> {
    type Source = crate::GuiContext<P>;
    type Target = ParamValue;

    fn view<'a>(&self, source: &'a Self::Source) -> Option<LensValue<'a, Self::Target>> {
        Some(LensValue::Borrowed(&source.params[self.param]))
    }
}
