use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{Empty, Enum};
use std::fmt::{Formatter, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::{fmt, ops};

pub use clack_extensions::params::ParamInfoFlags;

/// Mapping from and to a normalized range.
pub trait Mapping: Send + Sync {
    fn normalize(&self, value: f32) -> f32;
    fn denormalize(&self, value: f32) -> f32;
    fn range(&self) -> ops::Range<f32>;
}

pub trait MappingExt: Sized + Mapping {
    #[inline]
    fn as_dyn(&self) -> &dyn Mapping {
        self
    }

    #[inline]
    fn into_dyn(self) -> DynMapping
    where
        Self: 'static,
    {
        Arc::new(self)
    }
}

impl<M: Mapping> MappingExt for M {}

pub struct Linear;

impl Mapping for Linear {
    #[inline]
    fn normalize(&self, value: f32) -> f32 {
        value
    }

    #[inline]
    fn denormalize(&self, value: f32) -> f32 {
        value
    }

    fn range(&self) -> ops::Range<f32> {
        0.0..1.0
    }
}

pub struct Range<M> {
    pub inner: M,
    pub min: f32,
    pub max: f32,
}

impl<M: Mapping> Mapping for Range<M> {
    #[inline]
    fn normalize(&self, value: f32) -> f32 {
        self.inner.normalize((value - self.min) / (self.max - self.min))
    }

    #[inline]
    fn denormalize(&self, value: f32) -> f32 {
        self.min + (self.max - self.min) * self.inner.denormalize(value)
    }

    fn range(&self) -> ops::Range<f32> {
        self.min..self.max
    }
}

pub struct Polynomial {
    forward: f32,
    backward: f32,
}

impl Mapping for Polynomial {
    fn normalize(&self, value: f32) -> f32 {
        value.powf(self.backward)
    }

    fn denormalize(&self, value: f32) -> f32 {
        value.powf(self.forward)
    }

    fn range(&self) -> ops::Range<f32> {
        0.0..1.0
    }
}

impl Polynomial {
    pub const fn new(factor: f32) -> Self {
        Self {
            forward: factor,
            backward: factor.recip(),
        }
    }
}

pub type DynMapping = Arc<dyn Mapping>;

pub const fn linear(min: f32, max: f32) -> impl Mapping {
    Range {
        inner: Linear,
        min,
        max,
    }
}

pub fn polynomial_raw(min: f32, max: f32, factor: f32) -> impl Mapping {
    Range {
        inner: Polynomial::new(factor),
        min,
        max,
    }
}

pub fn polynomial(min: f32, max: f32, factor: f32) -> impl Mapping {
    polynomial_raw(min, max, factor.exp())
}

pub struct ParamValue {
    value: AtomicU32,
    changed: AtomicBool,
    mapping: DynMapping,
}

impl fmt::Debug for ParamValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for ParamValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}

impl ParamValue {
    const ORDERING: Ordering = Ordering::Relaxed;
    pub fn new(mapping: impl 'static + Mapping, value: f32) -> Self {
        Self::new_dyn(mapping.into_dyn(), value)
    }

    pub const fn new_dyn(mapping: DynMapping, value: f32) -> Self {
        Self {
            value: AtomicU32::new(value.to_bits()),
            mapping,
            changed: AtomicBool::new(true),
        }
    }

    pub fn new_normal(value: f32) -> Self {
        Self::new(Linear, value)
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.value.load(Self::ORDERING))
    }

    pub fn get_normalized(&self) -> f32 {
        self.mapping.normalize(self.get())
    }

    pub fn set(&self, value: f32) {
        self.value.store(value.to_bits(), Self::ORDERING);
        self.changed.store(true, Self::ORDERING);
    }

    pub fn set_normalized(&self, value: f32) {
        self.set(self.mapping.denormalize(value));
    }

    pub fn get_changed(&self) -> bool {
        self.changed.load(Self::ORDERING)
    }

    pub fn has_changed(&self) -> bool {
        self.changed.swap(false, Self::ORDERING)
    }
}

pub trait ParamId: Sync + Enum {
    fn text_to_value(&self, text: &str) -> Option<f32>;
    fn default_value(&self) -> f32;
    fn mapping(&self) -> DynMapping;
    fn value_to_text(&self, f: &mut dyn fmt::Write, denormalized: f32) -> fmt::Result;
    fn flags(&self) -> ParamInfoFlags;
}

#[derive(Debug, Clone)]
pub struct ParamStorage<E: Enum>(Arc<EnumMapArray<E, ParamValue>>);

impl<E: ParamId> Default for ParamStorage<E> {
    fn default() -> Self {
        Self(Arc::new(EnumMapArray::new(|p: E| {
            ParamValue::new_dyn(p.mapping(), p.default_value())
        })))
    }
}

impl<E: Enum> ParamStorage<E> {
    #[inline]
    pub fn get(&self, id: E) -> f32 {
        self.0[id].get()
    }

    #[inline]
    pub fn get_normalized(&self, id: E) -> f32 {
        self.0[id].get_normalized()
    }

    #[inline]
    pub fn read_all_values(&self) -> EnumMapArray<E, f32> {
        EnumMapArray::new(|p| self.0[p].get())
    }

    #[inline]
    pub fn set(&self, id: E, value: f32) {
        self.0[id].set(value);
    }

    #[inline]
    pub fn set_normalized(&self, id: E, value: f32) {
        self.0[id].set_normalized(value);
    }

    pub fn store_all_values(&self, values: EnumMapArray<E, f32>) {
        for (id, value) in values.iter() {
            self.0[id].set(*value);
        }
    }
}

impl ParamId for Empty {
    fn text_to_value(&self, _text: &str) -> Option<f32> {
        None
    }

    fn default_value(&self) -> f32 {
        unreachable!()
    }

    fn mapping(&self) -> DynMapping {
        unreachable!()
    }

    fn value_to_text(&self, _f: &mut dyn Write, _denormalized: f32) -> fmt::Result {
        unreachable!()
    }

    fn flags(&self) -> ParamInfoFlags {
        unreachable!()
    }
}
