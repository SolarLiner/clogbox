use std::marker::PhantomData;
use std::ops;
use std::rc::Rc;
use std::sync::Arc;
use duplicate::duplicate_item;
use crate::param::events::ParamEvents;
use crate::r#enum::Enum;
use crate::r#enum::enum_map::{Collection, EnumMap};

pub trait ParamEventsContainer<I> {
    fn get_param_events(&self, index: I) -> Option<&dyn ParamEvents>;
}

impl<I, C: ?Sized + ParamEventsContainer<I>> ParamEventsContainer<I> for &C {
    fn get_param_events(&self, index: I) -> Option<&dyn ParamEvents> {
        C::get_param_events(self, index)
    }
}

#[duplicate_item(
container;
[Box];
[Rc];
[Arc];
)]
impl<I, E: ?Sized + ParamEventsContainer<I>> ParamEventsContainer<I> for container<E> {
    fn get_param_events(&self, index: I) -> Option<&dyn ParamEvents> {
        E::get_param_events(self, index)
    }
}

impl<E: ParamEvents> ParamEventsContainer<usize> for [E] {
    fn get_param_events(&self, index: usize) -> Option<&dyn ParamEvents> {
        self.get(index).map(|e| e as &dyn ParamEvents)
    }
}

impl<I: Enum, C: Collection<Item=dyn ParamEvents>> ParamEventsContainer<I> for EnumMap<I, C> {
    fn get_param_events(&self, index: I) -> Option<&dyn ParamEvents> {
        Some(&self[index])
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MappedContainer<In, Out, F: Fn(Out) -> In, Inner: ParamEventsContainer<In>> {
    pub inner: Inner,
    mapping: F,
    __index: PhantomData<fn(Out) -> In>,
}

impl<In, Out, F: Fn(Out) -> In, Inner: ParamEventsContainer<In>> ParamEventsContainer<Out> for MappedContainer<In, Out, F, Inner> {
    fn get_param_events(&self, index: Out) -> Option<&dyn ParamEvents> {
        self.inner.get_param_events((self.mapping)(index))
    }
}

impl<In, Out, F: Fn(Out) -> In, Inner: ParamEventsContainer<In>> MappedContainer<In, Out, F, Inner> {
    pub fn new(inner: Inner, mapping: F) -> Self {
        Self {
            inner,
            mapping,
            __index: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::param::events::ParamSlice;
    use super::*;
    
    fn requires_param_events_container<I, C: ?Sized + ParamEventsContainer<I>>() {}
    
    #[test]
    fn check_types_impl() {
        requires_param_events_container::<usize, Box<[Box<ParamSlice>]>>();
    }
}