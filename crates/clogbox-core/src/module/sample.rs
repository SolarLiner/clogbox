//! This module provides an extensive framework for handling and processing audio streams.
//! It includes various data structures and types that facilitate the management of stream data,
//! the context for processing modules, and the status of ongoing processes.
//!
//! # Example
//!
//! ```rust
//! use std::marker::PhantomData;
//! use std::ops;
//! use std::sync::Arc;
//! use num_traits::Num;
//! use typenum::U1;
//! use clogbox_core::module::{Module, StreamData, ProcessStatus};
//! use clogbox_core::module::sample::{SampleContext, SampleContextImpl, SampleModule};
//! use clogbox_core::param::{Params, EMPTY_PARAMS};
//! use clogbox_core::r#enum::{enum_iter, seq, Empty, Enum, Sequential};
//! use clogbox_core::r#enum::enum_map::{EnumMapArray, EnumMapMut};
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
//!     type Params = Empty;
//!
//!     fn get_params(&self) -> Arc<impl '_ + Params<Params=Self::Params>> {
//!         Arc::new(EMPTY_PARAMS)
//!     }
//!
//!     fn latency(&self, input_latency: EnumMapArray<Self::Inputs, f64>) -> EnumMapArray<Self::Outputs, f64> {
//!         input_latency
//!     }
//!
//!     fn process_sample(&mut self, stream_data: &StreamData, inputs: EnumMapArray<Self::Inputs, Self::Sample>, mut outputs: EnumMapMut<Self::Outputs, Self::Sample>) -> ProcessStatus {
//!         for k in enum_iter() {
//!             outputs[k] = -inputs[k];
//!         }
//!         ProcessStatus::Running
//!     }
//! }
//!
//! let mut module = Inverter::<f32, Sequential<U1>>::default();
//! let stream_data = &StreamData {
//!     bpm: 120.,
//!     block_size: 1,
//!     sample_rate: 44100.,
//! };
//! let inputs = EnumMapArray::new(|_| 42.0);
//! let mut outputs = EnumMapArray::new(|_| 0.0);
//! let status = module.process_sample(stream_data, inputs, outputs.to_mut());
//! assert_eq!(ProcessStatus::Running, status);
//! assert_eq!(-42.0, outputs[seq(0)]);
//! ```

use std::sync::Arc;
use num_traits::Zero;
use crate::module::{Module, ModuleContext, ProcessStatus, StreamData};
use crate::r#enum::enum_map::{EnumMapArray, EnumMapMut, EnumMapRef};
use crate::r#enum::Enum;
use numeric_array::ArrayLength;
use crate::param::Params;
use super::BufferStorage;

/// Type alias for the sample context implementation,
/// making it easier to use with [`SampleModule`] implementations.
pub type SampleContext<'a, M> = SampleContextImpl<
    'a,
    <M as SampleModule>::Sample,
    <M as SampleModule>::Inputs,
    <M as SampleModule>::Outputs,
>;

/// A context implementation for handling stream data with input and output enums.
pub struct SampleContextImpl<'a, T, In: Enum, Out: Enum>
where
    In::Count: ArrayLength,
    Out::Count: ArrayLength,
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

    type Params: Enum;

    fn get_params(&self) -> Arc<impl '_ + Params<Params=Self::Params>>;

    /// Reallocate resources based on the provided stream data.
    #[inline]
    fn reallocate(&mut self, stream_data: StreamData) {}

    /// Reset the state of the module.
    #[inline]
    fn reset(&mut self) {}

    /// Calculate the output latency based on input latency.
    fn latency(
        &self,
        input_latency: EnumMapArray<Self::Inputs, f64>,
    ) -> EnumMapArray<Self::Outputs, f64>;

    /// Process the given context and update the status.
    fn process_sample(&mut self, stream_data: &StreamData, inputs: EnumMapArray<Self::Inputs, Self::Sample>, outputs: EnumMapMut<Self::Outputs, Self::Sample>) -> ProcessStatus;
    
    /// This method is run at the beginning of each block. This is provided to [SampleModule]s in
    /// order to allow per-block processing (e.g. updating coefficients, etc.).
    /// 
    /// # Arguments 
    /// 
    /// * `stream_data`: [StreamData] for this block.
    /// 
    /// returns: ProcessStatus 
    fn on_begin_block(&mut self, stream_data: &StreamData) -> ProcessStatus {
        ProcessStatus::Running
    }
}

#[profiling::all_functions]
impl<M: SampleModule<Sample: Copy + Zero>> Module for M {
    type Sample = M::Sample;
    type Inputs = M::Inputs;
    type Outputs = M::Outputs;
    type Params = M::Params;

    fn get_params(&self) -> Arc<impl '_ + Params<Params=Self::Params>> {
        M::get_params(self)
    }

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
    fn latency(
        &self,
        input_latency: EnumMapArray<Self::Inputs, f64>,
    ) -> EnumMapArray<Self::Outputs, f64>
    where
        <Self::Outputs as Enum>::Count: ArrayLength,
    {
        M::latency(self, input_latency)
    }

    fn process<S: BufferStorage<Sample=Self::Sample, Input=Self::Inputs, Output=Self::Outputs>>(
        &mut self,
        context: &mut ModuleContext<S>,
    ) -> ProcessStatus {
        let mut status = ProcessStatus::Running;
        let block_size = context.stream_data().block_size;
        for i in 0..block_size {
            let sample_in = EnumMapArray::new(|inp: Self::Inputs| context.buffers.get_input_buffer(inp)[i]);
            let mut sample_out = EnumMapArray::new(|_| Self::Sample::zero());
            let new_status = M::process_sample(self, context.stream_data(), sample_in, sample_out.to_mut());
            for (out, val) in sample_out {
                context.buffers.get_output_buffer(out)[i] = val;
            }

            status = status.merge(&new_status);
        }
        status
    }
}
