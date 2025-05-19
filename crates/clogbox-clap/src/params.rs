//! CLAP-specific traits and implementation for working with parameters
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{count, Empty, Enum};
use std::fmt::{Formatter, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::{fmt, ops};

pub use clack_extensions::params::ParamInfoFlags;
use clogbox_math::{db_to_linear, linear_to_db};
#[cfg(feature = "gui")]
use ringbuf::traits::{Consumer, Producer, Split};

/// Mapping from and to a normalized range.
pub trait Mapping: Send + Sync {
    /// Normalize the value from the mapping entire range down to `0..1`.
    fn normalize(&self, value: f32) -> f32;
    /// Map the normalized value back from `0..1` to the mapping's entire range.
    fn denormalize(&self, value: f32) -> f32;
    /// Range of this mapping
    fn range(&self) -> ops::Range<f32>;
}

/// Extension methods for [`Mapping`] types
pub trait MappingExt: Sized + Mapping {
    /// Type-erase this mapping
    #[inline]
    fn as_dyn(&self) -> &dyn Mapping {
        self
    }

    /// Type-erase this mapping, turning it into a shared [`Arc`] containing the mapping
    #[inline]
    fn into_dyn(self) -> DynMapping
    where
        Self: 'static,
    {
        Arc::new(self)
    }
}

impl<M: Mapping> MappingExt for M {}

/// Linear "no-op" mapping over the `0..1` range;
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

/// Ranged mapping, which takes the inner mapping and maps it to this new range.
pub struct Range<M> {
    /// Inner mapping
    pub inner: M,
    /// New minimum value
    pub min: f32,
    /// New maximum value
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

/// Polynomial normalized mapping (in the `0..1` range)
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

/// Type of type-erased, owned mappings (returned by [`MappingExt::into_dyn`])
pub type DynMapping = Arc<dyn Mapping>;

/// Instantiate a new ranged linear mapping (over `min..max`).
pub const fn linear(min: f32, max: f32) -> impl Mapping {
    Range {
        inner: Linear,
        min,
        max,
    }
}

/// Instantiate a new ranged polynomial mapping (over `min..max`). You might want [`polynomial`] which offers a more
/// useful range of `factor` values.
///
/// # Arguments
///
/// * `min`: Minimum range value
/// * `max`: Maximum range value
/// * `factor`: Polynomial remapping factor. 1 is equivalent to linear, < 1 expands the end of the range, and > 1
/// expands the end of the range. Negative values are not allowed.
pub fn polynomial_raw(min: f32, max: f32, factor: f32) -> impl Mapping {
    Range {
        inner: Polynomial::new(factor),
        min,
        max,
    }
}

/// Instantiate a new ranged polynomial mapping (over `min..max`). This is different from [`polynomial_raw`] in that
/// `factor` is first exponentiated to make its range more granular and well-balanced.
///
/// # Arguments
///
/// * `min`: Minimum range value
/// * `max`: Maximum range value
/// * `factor`: Polynomial remapping factor. 0 is equivalent to linear, < 0 expands the end of the range, and > 0
/// expands the end of the range.
pub fn polynomial(min: f32, max: f32, factor: f32) -> impl Mapping {
    polynomial_raw(min, max, factor.exp())
}

/// Logarithmic mapping
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

/// Instantiate a ranged logarithmic mapping (over `min..max`). The mapping is skewed by the logarithm towards the
/// beginning of the range.
///
/// This mapping does not work with negative (or zero) values.
///
/// # Arguments
///
/// * `base`: Logarithm base
/// * `min`: Minimum value
/// * `max`: Maximum value
pub fn logarithmic(base: f32, min: f32, max: f32) -> impl Mapping {
    Logarithmic {
        base,
        start: min.log(base),
        end: max.log(base),
    }
}

/// Instantiate a logarithmic mapping which represents a frequency range. The mapping is skewed logarithmically
/// towards the beginning as we have a logarithmic perception of frequencies.
///
/// This mapping does not work with negative (or zero) values.
///
/// # Arguments
///
/// * `min`: Minimum frequency
/// * `max`: Maximum frequency
pub fn frequency(min: f32, max: f32) -> impl Mapping {
    logarithmic(2.0, min, max)
}

/// Decibel (dB) mapping
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

/// Mapping of decibel (dB) values, internally represented in linear scale.
///
/// Decibels are logarithmic units that better represent the relationship between volumes humans percieve.
///
/// Even though decibels are logarithmic, this mapping supports negative values because they are handled and stored
/// (and exposed through the parameter) in linear units. Minus infinity, however, is still not possible.
///
/// # Arguments
///
/// * `min`: Minimum dB value
/// * `max`: Maximum dB value
pub fn decibel(min: f32, max: f32) -> impl Mapping {
    Decibel(Range {
        inner: Linear,
        min,
        max,
    })
}

/// Integer mapping which rounds the normalized values into the `min..max` range.
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

/// Instantiate a new integer mapping. This mapping rounds the values to the nearest integer.
///
/// # Arguments
///
/// * `min`: Minimum int value
/// * `max`: Maximum int value
pub fn int(min: i32, max: i32) -> impl Mapping {
    Int {
        min: min as f32,
        max: max as f32,
    }
}

/// Create a new integer mapping covering the range of `E`.
pub fn enum_<E: Enum>() -> impl Mapping {
    int(0, count::<E>() as i32 - 1)
}

/// Data contained in a [`ParamStorage`] instance, per parameter.
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
    /// Create a new instance.
    ///
    /// # Arguments
    ///
    /// * `mapping`: Parameter mapping
    /// * `value`: Current value
    pub fn new(mapping: impl 'static + Mapping, value: f32) -> Self {
        Self::new_dyn(mapping.into_dyn(), value)
    }

    /// Create a new instance with a type-erased mapping.
    ///
    /// # Arguments
    ///
    /// * `mapping`: Parameter mapping
    /// * `value`: Current value
    pub const fn new_dyn(mapping: DynMapping, value: f32) -> Self {
        Self {
            value: AtomicU32::new(value.to_bits()),
            mapping,
            changed: AtomicBool::new(true),
        }
    }

    /// Create a new normalized parameter, using a normalized mapping over 0..1.
    ///
    /// # Arguments
    ///
    /// * `value`: Current parameter value
    pub fn new_normalized(value: f32) -> Self {
        Self::new(Linear, value)
    }

    /// Get the current full-range value.
    pub fn get(&self) -> f32 {
        f32::from_bits(self.value.load(Self::ORDERING))
    }

    /// Get the current normalized value.
    pub fn get_normalized(&self) -> f32 {
        self.mapping.normalize(self.get())
    }

    /// Change the parameter given a full-range value.
    ///
    /// # Arguments
    ///
    /// * `value`: Full-range value, new value of the parameter.
    pub fn set(&self, value: f32) {
        self.value.store(value.to_bits(), Self::ORDERING);
        self.changed.store(true, Self::ORDERING);
    }

    /// Change the parameter given a normalized value.
    ///
    /// # Arguments
    ///
    /// * `value`: Normalized value, new value of the parameter.
    pub fn set_normalized(&self, value: f32) {
        self.set(self.mapping.denormalize(value));
    }

    /// Returns true if the value has changed since the last call to [`Self::has_changed`]. This method **does not**
    /// reset the changed flag.
    pub fn get_changed(&self) -> bool {
        self.changed.load(Self::ORDERING)
    }

    /// Returns true if the value has changed since we last called this method.
    ///
    /// If you want to check this flag without resetting it, use [`Self::get_changed`].
    pub fn has_changed(&self) -> bool {
        self.changed.swap(false, Self::ORDERING)
    }
}

/// Trait of [`Enum`]s which are used as parameter IDs in plugins.
///
/// All values are full-range.
pub trait ParamId: Sync + Enum {
    /// Parse the given text and output the corresponding value.
    fn text_to_value(&self, text: &str) -> Option<f32>;
    /// Return the default value of this parameter.
    fn default_value(&self) -> f32;
    /// Return the mapping of this parameter.
    fn mapping(&self) -> DynMapping;
    /// Write the text representation of the provided value of this parameter
    fn value_to_text(&self, f: &mut dyn fmt::Write, denormalized: f32) -> fmt::Result;

    /// Return true if this parameter can be automated, false otherwise.
    fn is_automatable(&self) -> bool {
        true
    }

    /// Return the number of steps this parameter has in the case of a discrete parameter; otherwise, return `None`.
    fn discrete(&self) -> Option<usize> {
        None
    }

    /// Formats a string of the text representation of the provided value of this parameter. This is equivalent to
    /// [`Self::value_to_text`] and should not need to be re-implemented.
    fn value_to_string(&self, denormalized: f32) -> Result<String, fmt::Error> {
        let mut buf = String::new();
        self.value_to_text(&mut buf, denormalized)?;
        Ok(buf)
    }
}

/// Extension trait of [`ParamId`] types.
pub trait ParamIdExt: ParamId {
    /// Return the computed CLAP parameter info flags based on the [`ParamId`] implementation.
    fn flags(&self) -> ParamInfoFlags {
        let mut flags = ParamInfoFlags::empty();
        flags.set(ParamInfoFlags::IS_AUTOMATABLE, self.is_automatable());
        flags.set(ParamInfoFlags::IS_STEPPED, self.discrete().is_some());
        flags
    }

    /// Convert a normalized parameter value to the corresponding CLAP value.
    ///
    /// CLAP values are the same as normalized values, except in the case of discrete parameters, where they are
    /// directly mapping to the discrete step.
    fn normalized_to_clap_value(&self, normalized: f32) -> f64 {
        if let Some(num_values) = self.discrete() {
            normalized as f64 * num_values as f64
        } else {
            normalized as f64
        }
    }

    /// Convert a full-range parameter value to the corresponding CLAP value.
    ///
    /// CLAP values are the same as normalized values, except in the case of discrete parameters, where they are
    /// directly mapping to the discrete step.
    fn denormalized_to_clap_value(&self, denormalized: f32) -> f64 {
        self.normalized_to_clap_value(self.mapping().normalize(denormalized))
    }

    /// Convert a CLAP value to the corresponding normalized value.
    ///
    /// CLAP values are the same as normalized values, except in the case of discrete parameters, where they are
    /// directly mapping to the discrete step.
    fn clap_value_to_normalized(&self, clap_value: f64) -> f32 {
        if let Some(num_values) = self.discrete() {
            clap_value as f32 / num_values as f32
        } else {
            clap_value as f32
        }
    }

    /// Convert a CLAP value to the corresponding full-range value.
    ///
    /// CLAP values are the same as normalized values, except in the case of discrete parameters, where they are
    /// directly mapping to the discrete step.
    fn clap_value_to_denormalized(&self, clap_value: f64) -> f32 {
        self.mapping().denormalize(self.clap_value_to_normalized(clap_value))
    }
}

impl<P: ParamId> ParamIdExt for P {}

/// Parameter events sent through from the GUI
#[derive(Debug, Copy, Clone)]
pub enum ParamChangeKind {
    /// A gesture has begun on the GUI
    GestureBegin,
    /// A gesture has ended on the GUI
    GestureEnd,
    /// The value of the parameter has changed
    ValueChange(f32),
}

/// Type of parameter events sent through from the GUI
#[derive(Debug, Copy, Clone)]
pub struct ParamChangeEvent<E> {
    /// Parameter ID
    pub id: E,
    /// Parameter change kind
    pub kind: ParamChangeKind,
}

/// Notify the DSP of parameter changes. The other side of a [`ParamListener`].
#[cfg(feature = "gui")]
#[derive(Clone)]
pub struct ParamNotifier<E> {
    producer: Arc<Mutex<ringbuf::HeapProd<ParamChangeEvent<E>>>>,
}

#[cfg(feature = "gui")]
impl<E> ParamNotifier<E> {
    /// Notify of a parameter change.
    ///
    /// # Arguments
    ///
    /// * `id`: Parameter ID
    /// * `kind`: Parameter change event
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

/// Parameter listener, the other side of a [`ParameterNotifier`].
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
    /// Return the last parameter value received by this listener if it has received one.
    ///
    /// # Arguments
    ///
    /// * `id`: Parameter ID
    pub fn value_of(&self, id: E) -> Option<f32> {
        Some(self.received_values[id]).filter(|v| !v.is_nan())
    }

    fn construct(consumer: ringbuf::HeapCons<ParamChangeEvent<E>>) -> Self {
        Self {
            consumer,
            received_values: EnumMapArray::new(|_| f32::NAN),
        }
    }
}

/// Create a parameter notifier/listener pair.
#[cfg(feature = "gui")]
pub fn create_notifier_listener<E: Enum>(capacity: usize) -> (ParamNotifier<E>, ParamListener<E>) {
    let (producer, consumer) = ringbuf::HeapRb::new(capacity).split();
    (ParamNotifier::construct(producer), ParamListener::construct(consumer))
}

/// Type of storage of parameters with their values and whether they have changed or not.
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
