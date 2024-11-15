#![warn(missing_docs)]
//! Implementation of non-linear filters.
//!
//! This module provides a number of non-linear filters that can be used to modify the
//! amplitude of audio signals.
use clogbox_core::module::sample::SampleModule;
use clogbox_core::module::{
    BufferStorage, Module, ModuleContext, ProcessStatus, StreamData,
};
use clogbox_core::r#enum::enum_map::{EnumMapArray, EnumMapMut};
use clogbox_core::r#enum::{seq, Sequential};
use num_traits::Float;
use std::marker::PhantomData;
use typenum::U1;

pub mod svf;

/// A trait representing a saturator that can saturate mono signals.
pub trait Saturator {
    /// The type of sample that the saturator works with.
    type Sample;

    /// Saturates a single value.
    ///
    /// # Parameters
    /// - `value`: The value to be saturated.
    ///
    /// # Returns
    /// The saturated value.
    fn saturate(&mut self, value: Self::Sample) -> Self::Sample;

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
        for value in buffer {
            *value = self.saturate(*value);
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

/// A module that encapsulates a saturator.
#[derive(Debug, Copy, Clone)]
pub struct SaturatorModule<S: Saturator>(pub S);

impl<Sat: 'static + Send + Saturator<Sample: Copy>> Module for SaturatorModule<Sat> {
    type Sample = Sat::Sample;
    type Inputs = Sequential<U1>;
    type Outputs = Sequential<U1>;

    fn supports_stream(&self, _: StreamData) -> bool {
        true
    }

    fn latency(
        &self,
        input_latencies: EnumMapArray<Self::Inputs, f64>,
    ) -> EnumMapArray<Self::Outputs, f64> {
        input_latencies
    }

    #[inline]
    #[profiling::function]
    fn process<
        S: BufferStorage<Sample = Self::Sample, Input = Self::Inputs, Output = Self::Outputs>,
    >(
        &mut self,
        context: &mut ModuleContext<S>,
    ) -> ProcessStatus {
        let (inp, out) = context.get_input_output_pair(seq(0), seq(0));
        self.0.saturate_buffer(inp, out);
        ProcessStatus::Running
    }
}

/// A [`SampleModule`] that holds a saturator which implements the [`Saturator`] trait.
#[derive(Debug, Copy, Clone)]
pub struct SaturatorSampleModule<S: Saturator>(pub S);

impl<S: 'static + Send + Saturator<Sample: Copy>> SampleModule for SaturatorSampleModule<S> {
    type Sample = S::Sample;
    type Inputs = Sequential<U1>;
    type Outputs = Sequential<U1>;

    fn latency(
        &self,
        input_latency: EnumMapArray<Self::Inputs, f64>,
    ) -> EnumMapArray<Self::Outputs, f64> {
        input_latency
    }

    fn process_sample(
        &mut self,
        _stream_data: &StreamData,
        inputs: EnumMapArray<Self::Inputs, Self::Sample>,
        mut outputs: EnumMapMut<Self::Outputs, Self::Sample>,
    ) -> ProcessStatus {
        outputs[seq(0)] = self.0.saturate(inputs[seq(0)]);
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

    #[inline]
    fn saturate(&mut self, value: Self::Sample) -> Self::Sample {
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

    #[inline]
    fn saturate(&mut self, value: Self::Sample) -> Self::Sample {
        self.1(value)
    }
}

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
