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
//! use clogbox_core::param::Params;
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
//!     fn process_sample(&mut self, context: SampleContext<Self>) -> ProcessStatus {
//!         for k in enum_iter() {
//!             context.outputs[k] = -context.inputs[k];
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
//! let context = SampleContextImpl {
//!     stream_data,
//!     inputs,
//!     outputs: outputs.to_mut(),
//!     params: EnumMapArray::new(|_| 0.0),
//! };
//! let status = module.process_sample(context);
//! assert_eq!(ProcessStatus::Running, status);
//! assert_eq!(-42.0, outputs[seq(0)]);
//! ```

use super::BufferStorage;
use crate::module::{Module, ModuleContext, ProcessStatus, StreamData};
use crate::param::events::{ParamEvents, ParamSlice};
use crate::param::Params;
use crate::r#enum::enum_map::{EnumMapArray, EnumMapMut, EnumMapRef};
use crate::r#enum::Enum;
use num_traits::Zero;
use numeric_array::ArrayLength;

/// Type alias for the sample context implementation,
/// making it easier to use with [`SampleModule`] implementations.
pub type SampleContext<'a, M> = SampleContextImpl<
    'a,
    <M as SampleModule>::Sample,
    <M as SampleModule>::Inputs,
    <M as SampleModule>::Outputs,
    <M as SampleModule>::Params,
>;

/// A context implementation for handling stream data with input and output enums.
pub struct SampleContextImpl<'a, T, In: Enum, Out: Enum, Params: Enum> {
    /// Reference to the stream data.
    pub stream_data: &'a StreamData,
    /// Enum map array for input data.
    pub inputs: EnumMapArray<In, T>,
    /// Enum map array for output data.
    pub outputs: EnumMapMut<'a, Out, T>,
    /// Enum map array for parameters.
    pub params: EnumMapArray<Params, f32>,
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

    /// Params type representing the input parameters of the module.
    type Params: Params;

    /// Reallocate resources based on the provided stream data.
    #[inline]
    fn reallocate(&mut self, stream_data: StreamData) {}

    /// Reset the state of the module.
    #[inline]
    fn reset(&mut self) {}

    /// Calculate the output latency based on input latency.
    fn latency(&self) -> f64 {
        0.0
    }

    /// Process the given context and update the status.
    fn process_sample(&mut self, context: SampleContext<Self>) -> ProcessStatus;

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
    fn latency(&self) -> f64 {
        M::latency(self)
    }

    fn process<
        S: BufferStorage<Sample = Self::Sample, Input = Self::Inputs, Output = Self::Outputs>,
    >(
        &mut self,
        context: &mut ModuleContext<S>,
        params: EnumMapRef<Self::Params, &dyn ParamEvents>,
    ) -> ProcessStatus {
        let mut status = ProcessStatus::Running;
        let block_size = context.stream_data().block_size;
        for i in 0..block_size {
            let inputs =
                EnumMapArray::new(|inp: Self::Inputs| context.buffers.get_input_buffer(inp)[i]);
            let mut outputs = EnumMapArray::new(|_| Self::Sample::zero());
            let params = EnumMapArray::new(|p| params[p].interpolate(i));
            let new_status = M::process_sample(
                self,
                SampleContextImpl {
                    stream_data: &context.stream_data,
                    params,
                    inputs,
                    outputs: outputs.to_mut(),
                },
            );
            for (out, val) in outputs {
                context.buffers.get_output_buffer(out)[i] = val;
            }

            status = status.merge(&new_status);
        }
        status
    }
}
