use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{count, Empty, Enum};
use std::fmt::{Formatter, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::{fmt, ops};

pub use clack_extensions::params::ParamInfoFlags;
use clack_plugin::events::io::InputEventBuffer;
use clogbox_math::{db_to_linear, linear_to_db};
#[cfg(feature = "gui")]
use ringbuf::traits::{Consumer, Producer, Split};

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

#[derive(Debug, Copy, Clone)]
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

#[derive(Debug, Copy, Clone)]
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

#[derive(Debug, Copy, Clone)]
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

pub const fn linear(min: f32, max: f32) -> Range<Linear> {
    Range {
        inner: Linear,
        min,
        max,
    }
}

pub fn polynomial_raw(min: f32, max: f32, factor: f32) -> Range<Polynomial> {
    Range {
        inner: Polynomial::new(factor),
        min,
        max,
    }
}

pub fn polynomial(min: f32, max: f32, factor: f32) -> Range<Polynomial> {
    polynomial_raw(min, max, factor.exp())
}

#[derive(Debug, Copy, Clone)]
pub struct Logarithmic {
    base: f32,
    start: f32,
    end: f32,
}

impl Mapping for Logarithmic {
    fn normalize(&self, value: f32) -> f32 {
        let x = value.log(self.base);
        (x - self.start) / (self.end - self.start)
    }

    fn denormalize(&self, value: f32) -> f32 {
        let x = self.start + (self.end - self.start) * value;
        self.base.powf(x)
    }

    fn range(&self) -> ops::Range<f32> {
        self.base.powf(self.start)..self.base.powf(self.end)
    }
}

pub fn logarithmic(base: f32, start: f32, end: f32) -> Logarithmic {
    Logarithmic {
        base,
        start: start.log(base),
        end: end.log(base),
    }
}

pub fn frequency(min: f32, max: f32) -> Logarithmic {
    logarithmic(2.0, min, max)
}

#[derive(Debug, Copy, Clone)]
pub struct Decibel(Range<Linear>);

impl Mapping for Decibel {
    fn normalize(&self, value: f32) -> f32 {
        self.0.normalize(linear_to_db(value))
    }

    fn denormalize(&self, value: f32) -> f32 {
        db_to_linear(self.0.denormalize(value))
    }

    fn range(&self) -> ops::Range<f32> {
        self.0.range()
    }
}

pub fn decibel(min: f32, max: f32) -> Decibel {
    Decibel(Range {
        inner: Linear,
        min,
        max,
    })
}

#[derive(Debug, Copy, Clone)]
pub struct Int {
    min: f32,
    max: f32,
}

impl Mapping for Int {
    fn normalize(&self, value: f32) -> f32 {
        (value.round() - self.min) / (self.max - self.min)
    }

    fn denormalize(&self, value: f32) -> f32 {
        (self.min + (self.max - self.min) * value).round()
    }

    fn range(&self) -> ops::Range<f32> {
        self.min..self.max
    }
}

pub fn int(min: i32, max: i32) -> Int {
    Int {
        min: min as f32,
        max: max as f32,
    }
}

pub fn enum_<E: Enum>() -> Int {
    int(0, count::<E>() as i32 - 1)
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

    fn is_automatable(&self) -> bool {
        true
    }
    fn discrete(&self) -> Option<usize> {
        None
    }

    fn value_to_string(&self, denormalized: f32) -> Result<String, fmt::Error> {
        let mut buf = String::new();
        self.value_to_text(&mut buf, denormalized)?;
        Ok(buf)
    }
}

pub trait ParamIdExt: ParamId {
    fn flags(&self) -> ParamInfoFlags {
        let mut flags = ParamInfoFlags::empty();
        flags.set(ParamInfoFlags::IS_AUTOMATABLE, self.is_automatable());
        flags.set(ParamInfoFlags::IS_STEPPED, self.discrete().is_some());
        flags
    }

    fn normalized_to_clap_value(&self, normalized: f32) -> f64 {
        if let Some(num_values) = self.discrete() {
            normalized as f64 * num_values as f64
        } else {
            normalized as f64
        }
    }

    fn denormalized_to_clap_value(&self, denormalized: f32) -> f64 {
        self.normalized_to_clap_value(self.mapping().normalize(denormalized))
    }

    fn clap_value_to_normalized(&self, clap_value: f64) -> f32 {
        if let Some(num_values) = self.discrete() {
            clap_value as f32 / num_values as f32
        } else {
            clap_value as f32
        }
    }

    fn clap_value_to_denormalized(&self, clap_value: f64) -> f32 {
        self.mapping().denormalize(self.clap_value_to_normalized(clap_value))
    }
}

impl<P: ParamId> ParamIdExt for P {}

#[derive(Debug, Copy, Clone)]
pub enum ParamChangeKind {
    GestureBegin,
    GestureEnd,
    ValueChange(f32),
}

#[derive(Debug, Copy, Clone)]
pub struct ParamChangeEvent<E> {
    pub id: E,
    pub kind: ParamChangeKind,
}

#[cfg(feature = "gui")]
#[derive(Clone)]
pub struct ParamNotifier<E> {
    producer: Arc<Mutex<ringbuf::HeapProd<ParamChangeEvent<E>>>>,
}

#[cfg(feature = "gui")]
impl<E> ParamNotifier<E> {
    pub fn notify(&self, id: E, kind: ParamChangeKind) {
        if self.get_producer().try_push(ParamChangeEvent { id, kind }).is_err() {
            log::debug!("ParamNotifier: ring buffer full");
        }
    }

    fn get_producer(&self) -> impl '_ + Drop + ops::DerefMut<Target = ringbuf::HeapProd<ParamChangeEvent<E>>> {
        match self.producer.lock() {
            Ok(p) => p,
            Err(err) => {
                log::debug!("ParamNotifier: Mutex poisoned, recovering: {err}");
                err.into_inner()
            }
        }
    }

    fn construct(producer: ringbuf::HeapProd<ParamChangeEvent<E>>) -> Self {
        Self {
            producer: Arc::new(Mutex::new(producer)),
        }
    }
}

#[cfg(feature = "gui")]
pub struct ParamListener<E: Enum> {
    consumer: ringbuf::HeapCons<ParamChangeEvent<E>>,
    received_values: EnumMapArray<E, f32>,
}

#[cfg(feature = "gui")]
impl<'a, E: Enum> Iterator for &'a mut ParamListener<E> {
    type Item = ParamChangeEvent<E>;
    fn next(&mut self) -> Option<ParamChangeEvent<E>> {
        self.consumer.try_pop()
    }
}

#[cfg(feature = "gui")]
impl<E: Enum> ParamListener<E> {
    pub fn value_of(&self, id: E) -> Option<f32> {
        Some(self.received_values[id]).filter(|v| !v.is_nan())
    }

    fn construct(consumer: ringbuf::HeapCons<ParamChangeEvent<E>>) -> Self {
        Self {
            consumer,
            received_values: EnumMapArray::new(|p| f32::NAN),
        }
    }
}

#[cfg(feature = "gui")]
pub fn create_notifier_listener<E: Enum>(capacity: usize) -> (ParamNotifier<E>, ParamListener<E>) {
    let (producer, consumer) = ringbuf::HeapRb::new(capacity).split();
    (ParamNotifier::construct(producer), ParamListener::construct(consumer))
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

impl<E: ParamId> ops::Index<E> for ParamStorage<E> {
    type Output = ParamValue;

    fn index(&self, index: E) -> &Self::Output {
        &self.0[index]
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
    pub fn get_clap_value(&self, id: E) -> f64
    where
        E: ParamId,
    {
        id.normalized_to_clap_value(self.get_normalized(id))
    }

    #[inline]
    pub fn read_all_values(&self) -> EnumMapArray<E, f32> {
        EnumMapArray::new(|p| self.0[p].get())
    }

    pub fn get_enum<E2: Enum>(&self, id: E) -> E2 {
        E2::from_usize(self.get(id).round() as _)
    }

    #[inline]
    pub fn set(&self, id: E, value: f32) {
        self.0[id].set(value);
    }

    #[inline]
    pub fn set_normalized(&self, id: E, value: f32) {
        self.0[id].set_normalized(value);
    }

    pub fn set_clap_value(&self, id: E, value: f64)
    where
        E: ParamId,
    {
        self.set_normalized(id, id.clap_value_to_normalized(value));
    }

    pub fn set_enum<E2: Enum>(&self, id: E, value: E2) {
        self.set_normalized(id, value.to_usize() as f32 / count::<E2>() as f32);
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
}
