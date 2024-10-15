#![warn(missing_docs)]

use std::marker::PhantomData;
use az::CastFrom;
use generic_array::ArrayLength;
use num_traits::Float;
use typenum::U1;
use clogbox_core::module::{Module, ProcessStatus, StreamData};
use clogbox_core::module::sample::{ModuleContext, SampleModule};
use clogbox_core::r#enum::{seq, Sequential, Enum};

pub mod svf;

pub trait Saturator {
    type Sample;
    
    fn saturate(&mut self, value: Self::Sample) -> Self::Sample;

    #[inline]
    #[profiling::function]
    fn saturate_buffer_in_place(&mut self, buffer: &mut [Self::Sample]) where Self::Sample: Copy {
        for value in buffer {
            *value = self.saturate(*value);
        }
    }
    
    #[inline]
    #[profiling::function]
    fn saturate_buffer(&mut self, input: &[Self::Sample], output: &mut [Self::Sample]) where Self::Sample: Copy {
        output.copy_from_slice(input);
        self.saturate_buffer_in_place(output);
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SaturatorModule<S: Saturator>(pub S);

impl<S:'static + Send + Saturator<Sample: Copy>> Module for SaturatorModule<S> {
    type Sample = S::Sample;
    type Inputs = Sequential<U1>;
    type Outputs = Sequential<U1>;

    fn supports_stream(&self, _: StreamData) -> bool {
        true
    }

    #[inline]
    #[profiling::function]
    fn process(&mut self, context: &mut clogbox_core::module::ModuleContext<Self>) -> ProcessStatus {
        self.0.saturate_buffer(context.input(seq(0)), context.output(seq(0)));
        ProcessStatus::Running
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SaturatorSampleModule<S: Saturator>(pub S);

impl<S: 'static + Send + Saturator<Sample: Copy>> SampleModule for SaturatorSampleModule<S> {
    type Sample = S::Sample;
    type Inputs = Sequential<U1>;
    type Outputs = Sequential<U1>;

    fn process(&mut self, context: &mut ModuleContext<Self>) -> ProcessStatus
    where
        <Self::Inputs as Enum>::Count: ArrayLength,
        <Self::Outputs as Enum>::Count: ArrayLength
    {
        context.outputs[seq(0)] = self.0.saturate(context.inputs[seq(1)]);
        ProcessStatus::Running
    }
}

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

#[derive(Debug, Copy, Clone)]
pub struct Memoryless<T, F>(PhantomData<T>, F);

impl<T, F> Memoryless<T, F> {
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

pub const fn tanh<T: Float>() -> Memoryless<T, fn(T) -> T> {
    Memoryless::new(T::tanh)
}

pub const fn asinh<T: Float>() -> Memoryless<T, fn(T) -> T> {
    Memoryless::new(T::asinh)
}

pub fn hard_clip<T: Float>(min: T, max: T) -> Memoryless<T, impl Copy + Fn(T) -> T> {
    Memoryless::new(move |x: T| x.clamp(min, max))
}