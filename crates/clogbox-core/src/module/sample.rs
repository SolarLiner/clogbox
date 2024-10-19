//! This module provides an extensive framework for handling and processing audio streams.
//! It includes various data structures and types that facilitate the management of stream data,
//! the context for processing modules, and the status of ongoing processes.
//!
//! # Example
//!
//! ```rust
//! use std::marker::PhantomData;
//! use std::ops;
//! use num_traits::Num;
//! use typenum::U1;
//! use clogbox_core::module::{ModuleContext, Module, StreamData, ProcessStatus};
//! use clogbox_core::module::sample::{SampleContext, SampleContextImpl, SampleModule};
//! use clogbox_core::r#enum::{enum_iter, seq, Enum, Sequential};
//! use clogbox_core::r#enum::enum_map::EnumMapArray;
//!
//! struct Inverter<T, In>(PhantomData<(T, In)>);
//!
//! impl<T, In> Default for Inverter <T, In>  {
//!     fn default() -> Self {
//!         Self(PhantomData)
//!     }
//! }
//!
//! impl<T: 'static + Send + Copy + Num + ops::Neg<Output=T>, In: 'static + Send + Enum> SampleModule for Inverter<T, In> {
//!     type Sample = T;
//!     type Inputs = In;
//!     type Outputs = In;
//!
//!     fn latency(&self, input_latency: EnumMapArray<Self::Inputs, f64>) -> EnumMapArray<Self::Outputs, f64> {
//!         input_latency
//!     }
//!
//!     fn process_sample(&mut self, context: &mut SampleContext<Self>) -> ProcessStatus {
//!         for inp in enum_iter::<In>() {
//!             context.outputs[inp] = -context.inputs[inp];
//!         }
//!         ProcessStatus::Running
//!     }
//! }
//!
//! let mut module = Inverter::<f32, Sequential<U1>>::default();
//! let mut context = SampleContextImpl {
//!     stream_data: &StreamData {
//!         bpm: 120.,
//!         block_size: 1,
//!         sample_rate: 44100.,
//!     },
//!     inputs: EnumMapArray::new(|_| 42.0),
//!     outputs: EnumMapArray::new(|_| 0.0),
//! };
//! assert_eq!(ProcessStatus::Running, module.process_sample(&mut context));
//! assert_eq!(-42.0, context.outputs[seq(0)]);
//! ```
use crate::module::{Module, ModuleContext as BaseModuleContext, ProcessStatus, StreamData};
use crate::r#enum::Enum;
use num_traits::Zero;
use numeric_array::ArrayLength;
use crate::r#enum::enum_map::EnumMapArray;

/// Type alias for the sample context implementation,
/// making it easier to use with [`SampleModule`] implementations.
pub type SampleContext<'a, M> = SampleContextImpl<'a, <M as SampleModule>::Sample, <M as SampleModule>::Inputs, <M as SampleModule>::Outputs>;

/// A context implementation for handling stream data with input and output enums.
pub struct SampleContextImpl<'a, T, In: Enum, Out: Enum>
where
    In::Count: ArrayLength,
    Out::Count: ArrayLength
{
    /// Reference to the stream data.
    pub stream_data: &'a StreamData,
    /// Enum map array for input data.
    pub inputs: EnumMapArray<In, T>,
    /// Enum map array for output data.
    pub outputs: EnumMapArray<Out, T>,
}

/// This trait outlines the module structure for per-sample handling and processing.
#[allow(unused_variables)]
pub trait SampleModule: 'static + Send {
    /// Represents the sample type used within the module.
    type Sample;

    /// Enum type representing inputs of the module.
    type Inputs: Enum;

    /// Enum type representing outputs of the module.
    type Outputs: Enum;

    /// Reallocate resources based on the provided stream data.
    #[inline]
    fn reallocate(&mut self, stream_data: StreamData) {}

    /// Reset the state of the module.
    #[inline]
    fn reset(&mut self) {}

    /// Calculate the output latency based on input latency.
    fn latency(&self, input_latency: EnumMapArray<Self::Inputs, f64>) -> EnumMapArray<Self::Outputs, f64>;

    /// Process the given context and update the status.
    fn process_sample(&mut self, context: &mut SampleContext<Self>) -> ProcessStatus
    where
        <Self::Inputs as Enum>::Count: ArrayLength,
        <Self::Outputs as Enum>::Count: ArrayLength;
}

#[profiling::all_functions]
impl<M: SampleModule<Sample: Copy + Zero>> Module for M {
    type Sample = M::Sample;
    type Inputs = M::Inputs;
    type Outputs = M::Outputs;

    #[inline]
    fn supports_stream(&self, _: StreamData) -> bool {
        true
    }

    #[inline]
    fn reallocate(&mut self, stream_data: StreamData) {
        M::reallocate(self, stream_data)
    }

    #[inline]
    fn reset(&mut self) {
        M::reset(self)
    }

    #[inline]
    fn latency(&self, input_latency: EnumMapArray<Self::Inputs, f64>) -> EnumMapArray<Self::Outputs, f64>
    where <Self::Outputs as Enum>::Count: ArrayLength {
        M::latency(self, input_latency)
    }

    fn process(&mut self, context: &mut BaseModuleContext<Self>) -> ProcessStatus {
        let mut status = ProcessStatus::Running;
        for i in 0..context.stream_data.block_size {
            let inputs = EnumMapArray::new(|inp| context.inputs[inp][i]);
            let outputs = EnumMapArray::new(|_| M::Sample::zero());
            let mut sample_ctx = SampleContextImpl {
                stream_data: context.stream_data,
                inputs,
                outputs,
            };
            let new_status = M::process_sample(self, &mut sample_ctx);
            for (out, val) in sample_ctx.outputs.iter() {
                context.outputs[out][i] = *val;
            }
            
            match new_status {
                ProcessStatus::Running => {}
                new_status @ ProcessStatus::Tail(_) => {
                    status = new_status;
                }
                ProcessStatus::Done => {
                    return ProcessStatus::Done;
                }
            }
        }
        status
    }
}