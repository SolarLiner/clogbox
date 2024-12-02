#![warn(missing_docs)]
//! Implementation of non-linear filters.
//!
//! This module provides a number of non-linear filters that can be used to modify the
//! amplitude of audio signals.
use az::CastFrom;
use clogbox_core::module::sample::{SampleContext, SampleModule};
use clogbox_core::module::{BufferStorage, Module, ModuleContext, ProcessStatus, StreamData};
use clogbox_core::param::events::ParamEvents;
use clogbox_core::param::Params;
use clogbox_core::r#enum::enum_map::{EnumMapArray, EnumMapMut, EnumMapRef};
use clogbox_core::r#enum::{seq, Empty, Enum, Mono, Sequential};
use clogbox_derive::{Enum, Params};
use num_traits::{Float, Num};
use std::marker::PhantomData;
use std::sync::Arc;
use typenum::U1;

pub mod svf;

/// A trait representing a saturator that can saturate mono signals.
pub trait Saturator {
    /// The type of sample that the saturator works with.
    type Sample;
    type Params: Params;

    /// Saturates a single value.
    ///
    /// # Parameters
    ///
    /// - `value`: The value to be saturated.
    ///
    /// # Returns
    ///
    /// The saturated value.
    fn saturate(
        &mut self,
        params: EnumMapArray<Self::Params, f32>,
        value: Self::Sample,
    ) -> Self::Sample;

    /// Saturates a buffer of values in place.
    ///
    /// # Parameters
    /// - `buffer`: The buffer containing the values to be saturated.
    #[inline]
    #[profiling::function]
    fn saturate_buffer_in_place(
        &mut self,
        params: EnumMapRef<Self::Params, &dyn ParamEvents>,
        buffer: &mut [Self::Sample],
    ) where
        Self::Sample: Copy,
    {
        for (i, value) in buffer.iter_mut().enumerate() {
            let params = EnumMapArray::new(|p| params[p].interpolate(i));
            *value = self.saturate(params, *value);
        }
    }

    /// Saturates a buffer of values, storing the results in an output buffer.
    ///
    /// # Parameters
    /// - `input`: The input buffer containing the values to be saturated.
    /// - `output`: The output buffer where the saturated values will be stored.
    #[inline]
    #[profiling::function]
    fn saturate_buffer(
        &mut self,
        params: EnumMapRef<Self::Params, &dyn ParamEvents>,
        input: &[Self::Sample],
        output: &mut [Self::Sample],
    ) where
        Self::Sample: Copy,
    {
        output.copy_from_slice(input);
        self.saturate_buffer_in_place(params, output);
    }
}

/// A module that encapsulates a saturator.
#[derive(Debug, Copy, Clone)]
pub struct SaturatorModule<S: Saturator>(pub S);

impl<Sat: 'static + Send + Saturator<Sample: Copy>> Module for SaturatorModule<Sat> {
    type Sample = Sat::Sample;
    type Inputs = Sequential<U1>;
    type Outputs = Sequential<U1>;
    type Params = Sat::Params;

    fn supports_stream(&self, _: StreamData) -> bool {
        true
    }

    fn latency(&self) -> f64 {
        0.0
    }

    #[inline]
    #[profiling::function]
    fn process<
        S: BufferStorage<Sample = Self::Sample, Input = Self::Inputs, Output = Self::Outputs>,
    >(
        &mut self,
        context: &mut ModuleContext<S>,
        params: EnumMapRef<Self::Params, &dyn ParamEvents>,
    ) -> ProcessStatus {
        let (inp, out) = context.get_input_output_pair(seq(0), seq(0));
        self.0.saturate_buffer(params, inp, out);
        ProcessStatus::Running
    }
}

/// A [`SampleModule`] that holds a saturator which implements the [`Saturator`] trait.
#[derive(Debug, Copy, Clone)]
pub struct SaturatorSampleModule<S: Saturator>(pub S);

impl<S: 'static + Send + Saturator<Sample: Copy>> SampleModule for SaturatorSampleModule<S> {
    type Sample = S::Sample;
    type Inputs = Mono;
    type Outputs = Mono;
    type Params = S::Params;

    fn latency(&self) -> f64 {
        0.0
    }

    fn process_sample(&mut self, mut context: SampleContext<Self>) -> ProcessStatus {
        use self::Mono::Mono;

        context.outputs[Mono] = self.0.saturate(context.params, context.inputs[Mono]);
        ProcessStatus::Running
    }
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
    fn saturate(
        &mut self,
        _: EnumMapArray<Self::Params, f32>,
        value: Self::Sample,
    ) -> Self::Sample {
        value
    }
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
    fn saturate(
        &mut self,
        _: EnumMapArray<Self::Params, f32>,
        value: Self::Sample,
    ) -> Self::Sample {
        self.1(value)
    }
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

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum, Params)]
pub enum DrivenParams<SatParams> {
    Drive,
    Inner(SatParams),
}

#[derive(Debug, Copy, Clone)]
pub struct Driven<Sat: Saturator>(pub Sat);

impl<Sat: Saturator<Sample: Copy + Num + CastFrom<f32>>> Saturator for Driven<Sat>
where
    DrivenParams<Sat::Params>: Params,
{
    type Sample = Sat::Sample;
    type Params = DrivenParams<Sat::Params>;

    fn saturate(
        &mut self,
        params: EnumMapArray<Self::Params, f32>,
        value: Self::Sample,
    ) -> Self::Sample {
        let amp = Sat::Sample::cast_from(params[DrivenParams::Drive]);
        self.0.saturate(
            EnumMapArray::new(|p| params[DrivenParams::Inner(p)]),
            amp * value,
        ) / amp
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum, Params)]
pub enum BiasedParams<SatParams> {
    Bias,
    Inner(SatParams),
}

#[derive(Debug, Copy, Clone)]
pub struct Biased<Sat: Saturator>(pub Sat);

impl<Sat: Saturator<Sample: Copy + Num + CastFrom<f32>>> Saturator for Biased<Sat>
where
    BiasedParams<Sat::Params>: Params,
{
    type Sample = Sat::Sample;
    type Params = BiasedParams<Sat::Params>;

    fn saturate(
        &mut self,
        params: EnumMapArray<Self::Params, f32>,
        value: Self::Sample,
    ) -> Self::Sample {
        let bias = Sat::Sample::cast_from(params[BiasedParams::Bias]);
        self.0.saturate(
            EnumMapArray::new(|p| params[BiasedParams::Inner(p)]),
            value + bias,
        ) - bias
    }
}
