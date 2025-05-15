#![warn(missing_docs)]
//! Implementation of non-linear filters.
//!
//! This module provides a number of non-linear filters that can be used to modify the
//! amplitude of audio signals.
use az::CastFrom;
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{Empty, Enum};
use num_traits::{Float, Num};
use std::marker::PhantomData;

pub mod biquad;
pub mod svf;

/// A trait representing a saturator that can saturate mono signals.
pub trait Saturator {
    /// The type of sample that the saturator works with.
    type Sample;
    type Params: Enum;

    /// Saturates a single value.
    ///
    /// # Parameters
    ///
    /// - `value`: The value to be saturated.
    ///
    /// # Returns
    ///
    /// The saturated value.
    fn saturate(&mut self, value: Self::Sample) -> Self::Sample;

    fn set_param(&mut self, param: Self::Params, value: f32);

    /// Saturates a buffer of values in place.
    ///
    /// # Parameters
    /// - `buffer`: The buffer containing the values to be saturated.
    #[inline]
    #[profiling::function]
    fn saturate_buffer_in_place(&mut self, buffer: &mut [Self::Sample])
    where
        Self::Sample: Copy,
    {
        for buf in buffer.iter_mut() {
            *buf = self.saturate(*buf);
        }
    }

    /// Saturates a buffer of values, storing the results in an output buffer.
    ///
    /// # Parameters
    /// - `input`: The input buffer containing the values to be saturated.
    /// - `output`: The output buffer where the saturated values will be stored.
    #[inline]
    #[profiling::function]
    fn saturate_buffer(&mut self, input: &[Self::Sample], output: &mut [Self::Sample])
    where
        Self::Sample: Copy,
    {
        output.copy_from_slice(input);
        self.saturate_buffer_in_place(output);
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum SaturatorInputs<P> {
    AudioInput,
    Params(P),
}

/// A "no-op" saturator. This saturator does not modify the input signal.
#[derive(Debug, Copy, Clone)]
pub struct Linear<T>(PhantomData<T>);

impl<T> Default for Linear<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> Saturator for Linear<T> {
    type Sample = T;
    type Params = Empty;

    #[inline]
    fn saturate(&mut self, value: Self::Sample) -> Self::Sample {
        value
    }

    #[inline]
    fn set_param(&mut self, _param: Self::Params, _value: f32) {}
}

/// A [`Saturator`] that can use any memoryless function to saturate the signal.
#[derive(Debug, Copy, Clone)]
pub struct Memoryless<T, F>(PhantomData<T>, F);

impl<T, F> Memoryless<T, F> {
    /// Creates a new [`Memoryless`] instance with the provided function.
    ///
    /// # Parameters
    ///
    /// - `f`: The function to be used by the [`Memoryless`] instance.
    ///
    /// # Returns
    ///
    /// A new [`Memoryless`] instance.
    pub const fn new(f: F) -> Memoryless<T, F> {
        Self(PhantomData, f)
    }
}

impl<T: Copy + Send, F: Send + Fn(T) -> T> Saturator for Memoryless<T, F> {
    type Sample = T;
    type Params = Empty;

    #[inline]
    fn saturate(&mut self, value: Self::Sample) -> Self::Sample {
        self.1(value)
    }

    #[inline]
    fn set_param(&mut self, _param: Self::Params, _value: f32) {}
}

pub type SimpleSaturator<T> = Memoryless<T, fn(T) -> T>;

/// Creates a new `Memoryless` instance using the hyperbolic tangent function.
///
/// # Returns
///
/// A new `Memoryless` instance for the `tanh` function.
pub const fn tanh<T: Float>() -> Memoryless<T, fn(T) -> T> {
    Memoryless::new(T::tanh)
}

/// Creates a `Memoryless` instance for the hyperbolic arcsine function.
///
/// # Returns
///
/// A `Memoryless` instance that uses the `asinh` function.
pub const fn asinh<T: Float>() -> Memoryless<T, fn(T) -> T> {
    Memoryless::new(T::asinh)
}

/// Creates a `Memoryless` instance for the hyperbolic sine function.
///
/// # Returns
///
/// A `Memoryless` instance that uses the `asinh` function.
pub const fn sinh<T: Float>() -> Memoryless<T, fn(T) -> T> {
    Memoryless::new(T::sinh)
}

/// Creates a `Memoryless` instance that clamps input values between `min` and `max`.
///
/// # Parameters
///
/// - `min`: The minimum value of the clamp range.
/// - `max`: The maximum value of the clamp range.
///
/// # Returns
///
/// A `Memoryless` instance that clamps input values.
pub fn hard_clip<T: Float>(min: T, max: T) -> Memoryless<T, impl Copy + Fn(T) -> T> {
    Memoryless::new(move |x: T| x.clamp(min, max))
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum DrivenParams<SatParams> {
    Drive,
    Inner(SatParams),
}

#[derive(Debug, Clone)]
pub struct Driven<Sat: Saturator>
where
    DrivenParams<Sat::Params>: Enum,
{
    pub saturator: Sat,
    pub params: EnumMapArray<DrivenParams<Sat::Params>, Sat::Sample>,
}

impl<Sat: Saturator<Sample: Copy + Num + CastFrom<f32>>> Saturator for Driven<Sat>
where
    DrivenParams<Sat::Params>: Enum,
{
    type Sample = Sat::Sample;
    type Params = DrivenParams<Sat::Params>;

    fn saturate(&mut self, value: Self::Sample) -> Self::Sample {
        let amp = self.params[DrivenParams::Drive];
        self.saturator.saturate(amp * value) / amp
    }

    fn set_param(&mut self, param: Self::Params, value: f32) {
        self.params[param] = Sat::Sample::cast_from(value);
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum BiasedParams<SatParams> {
    Bias,
    Inner(SatParams),
}

#[derive(Debug, Clone)]
pub struct Biased<Sat: Saturator>
where
    BiasedParams<Sat::Params>: Enum,
{
    pub saturator: Sat,
    pub params: EnumMapArray<BiasedParams<Sat::Params>, Sat::Sample>,
}

impl<Sat: Saturator<Sample: Copy + Num + CastFrom<f32>>> Saturator for Biased<Sat>
where
    BiasedParams<Sat::Params>: Enum,
{
    type Sample = Sat::Sample;
    type Params = BiasedParams<Sat::Params>;

    fn saturate(&mut self, value: Self::Sample) -> Self::Sample {
        let bias = self.params[BiasedParams::Bias];
        self.saturator.saturate(value + bias) - bias
    }

    fn set_param(&mut self, param: Self::Params, value: f32) {
        self.params[param] = Sat::Sample::cast_from(value);
    }
}
