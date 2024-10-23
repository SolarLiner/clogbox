//! This module provides core functionalities related to analysis within a system.
//!
//! It includes definitions and implementations crucial for processing and handling
//! various types of data streams. The functionalities are generalized and extensible
//! to support a wide range of input and output configurations, ensuring flexibility
//! and scalability in use.
//!
//! ## Example
//!
//! ```rust
//! use std::marker::PhantomData;
//! use std::ops;
//! use az::CastFrom;
//! use num_traits::Zero;
//! use numeric_array::generic_array::arr;
//! use typenum::U1;
//!
//! use clogbox_core::module::{StreamData, ProcessStatus, Module};
//! use clogbox_core::r#enum::{enum_iter, Enum, Sequential};
//! use clogbox_core::r#enum::enum_map::{Collection, CollectionMut, EnumMap, EnumMapArray, EnumMapMut, EnumMapRef};
//!
//! struct Inverter<T, In>(PhantomData<(T, In)>);
//!
//!
//! impl<T, In> Default for Inverter <T, In>  {
//!     fn default() -> Self {
//!         Self(PhantomData)
//!     }
//! }
//!
//! impl<T: 'static + Send + Copy + CastFrom<f64> + ops::Neg<Output=T>, In: 'static + Send + Enum> Module for Inverter<T, In> {
//!     type Sample = T;
//!     type Inputs = In;
//!     type Outputs = In;
//!
//!     fn supports_stream(&self, data: StreamData) -> bool {
//!         true
//!     }
//!
//!     fn latency(&self, input_latency: EnumMapArray<Self::Inputs, f64>) -> EnumMapArray<Self::Outputs, f64> {
//!         input_latency
//!     }
//!
//!     fn process(&mut self, stream_data: &StreamData, inputs: &[&[Self::Sample]], outputs: &mut [&mut [Self::Sample]]) -> ProcessStatus {
//!         for inp in enum_iter::<In>() {
//!             let inp_buf = &*inputs[inp.cast()];
//!             let out_buf = &mut *outputs[inp.cast()];
//!
//!             for (o, i) in out_buf.iter_mut().zip(inp_buf.iter()) {
//!                 *o = -*i;
//!             }   
//!         }       
//!         ProcessStatus::Running
//!     }
//! }
//!
//! let mut my_module = Inverter::<f32, Sequential<U1>>::default();
//! let block_size = 128;
//! let stream_data = StreamData { sample_rate: 44100.0 ,bpm: 120. ,block_size };
//! let inputs = (0..block_size).map(|i| i as f32).collect::<Vec<_>>();
//! let mut outputs = vec![0.0; block_size];
//! my_module.process(&stream_data, &[&inputs], &mut [&mut outputs]);
//! assert_eq!(-4., outputs[4]);
//! ```
pub mod analysis;
pub mod sample;
pub mod utilitarian;

use crate::r#enum::enum_map::EnumMapArray;
use crate::r#enum::{Enum, EnumIndex};
use az::Cast;
use std::marker::PhantomData;
use std::ops;
use typenum::Unsigned;

/// Represents the metadata and configuration for a stream of audio data.
#[derive(Debug, Copy, Clone)]
pub struct StreamData {
    /// The sample rate of the audio stream, in samples per second.
    pub sample_rate: f64,
    /// The beats per minute (BPM) of the audio stream.
    pub bpm: f64,
    /// The size of a processing block in samples.
    pub block_size: usize,
}

impl StreamData {
    /// Calculates the time duration of one sample in seconds.
    ///
    /// # Returns
    ///
    /// The time duration of one sample as a `f64` value.
    /// Calculates the time duration of one sample in seconds.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::module::StreamData;
    /// let stream_data = StreamData {
    ///     sample_rate: 44100.0,
    ///     bpm: 120.0,
    ///     block_size: 512,
    /// };
    /// let time_duration = stream_data.dt();
    /// assert_eq!(1./44100., time_duration);
    /// ```
    pub fn dt(&self) -> f64 {
        self.sample_rate.recip()
    }

    /// Calculates the length of a given number of beats in minutes.
    ///
    /// # Arguments
    ///
    /// * `beats` - The number of beats to calculate the length for.
    ///
    /// # Returns
    ///
    /// The length of the specified number of beats in minutes as a `f64` value.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::module::StreamData;
    /// let stream_data = StreamData {
    ///     sample_rate: 44100.0,
    ///     bpm: 120.0,
    ///     block_size: 512,
    /// };
    /// let beats = 4.0;
    /// let length = stream_data.beat_length(beats);
    /// assert_eq!(2.0, length);
    /// ```
    pub fn beat_length(&self, beats: f64) -> f64 {
        beats * 60. / self.bpm
    }

    /// Calculates the length of a given number of beats in samples.
    ///
    /// # Arguments
    /// * `beats` - The number of beats to calculate the length for.
    ///
    /// # Returns
    /// The length of the specified number of beats in samples as a `f64` value.
    pub fn beat_sample_length(&self, beats: f64) -> f64 {
        self.sample_rate * self.beat_length(beats)
    }
}

/// A trait representing a raw module with audio processing capabilities.
#[allow(unused_variables)]
pub trait RawModule: Send {
    /// The type of the samples processed by the module.
    type Sample;

    /// Returns the number of inputs of the module.
    fn inputs(&self) -> usize;

    /// Returns the number of outputs of the module.
    fn outputs(&self) -> usize;

    /// Checks if the module supports the given stream data.
    ///
    /// # Arguments
    ///
    /// * `data` - The stream data to check.
    fn supports_stream(&self, data: StreamData) -> bool;

    /// Reallocates resources based on the provided stream data.
    ///
    /// # Arguments
    ///
    /// * `stream_data` - The new stream data.
    fn reallocate(&mut self, stream_data: StreamData) {}

    /// Resets the module to its initial state.
    fn reset(&mut self) {}

    /// Processes the module with the given context.
    ///
    /// # Arguments
    ///
    /// * `context` - The context for processing.
    ///
    /// # Returns
    ///
    /// The status of the processing.
    fn process(
        &mut self,
        stream_data: &StreamData,
        inputs: &[&[Self::Sample]],
        outputs: &mut [&mut [Self::Sample]],
    ) -> ProcessStatus;
}

/// Represents the status of a process.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProcessStatus {
    /// The process is currently running.
    Running,
    /// The process will begin returning silence after this many samples, provided the input is also
    /// silent.
    Tail(u64),
    /// The process is completed.
    Done,
}

impl ProcessStatus {
    /// Merge two [`ProcessStatus`] instances, preserving as much information as possible.
    /// 
    /// # Arguments 
    /// 
    /// * `other`: A reference to another [`ProcessStatus`] instance that will be merged with `self`.
    /// 
    /// returns: ProcessStatus 
    /// 
    /// # Examples 
    /// 
    /// ```
    /// use clogbox_core::module::ProcessStatus;
    ///
    /// let status1 = ProcessStatus::Running;
    /// let status2 = ProcessStatus::Done;
    /// let merged_status = status1.merge(&status2);
    /// assert_eq!(merged_status, ProcessStatus::Done);
    ///
    /// let status3 = ProcessStatus::Tail(5);
    /// let status4 = ProcessStatus::Tail(10);
    /// let merged_tail_status = status3.merge(&status4);
    /// assert_eq!(merged_tail_status, ProcessStatus::Tail(15));
    ///
    /// let status5 = ProcessStatus::Running;
    /// let status6 = ProcessStatus::Running;
    /// let merged_running_status = status5.merge(&status6);
    /// assert_eq!(merged_running_status, ProcessStatus::Running);
    /// ```
    pub fn merge(&self, other: &Self) -> Self {
        match (self, other) {
            (ProcessStatus::Tail(a), ProcessStatus::Tail(b)) => ProcessStatus::Tail(a + b),
            (&ProcessStatus::Tail(a), _) | (_, &ProcessStatus::Tail(a)) => ProcessStatus::Tail(a),
            (ProcessStatus::Done, _) | (_, ProcessStatus::Done) => ProcessStatus::Done,
            _ => ProcessStatus::Running,
        }
    }
}

/// A module trait defining the basic functionalities and requirements for audio modules.
#[allow(unused_variables)]
pub trait Module: 'static + Send {
    /// The type representing a sample in the module.
    type Sample;

    /// The type representing the inputs of the module.
    type Inputs: Enum;

    /// The type representing the outputs of the module.
    type Outputs: Enum;

    /// Checks if the module supports the provided stream data.
    ///
    /// # Arguments
    ///
    /// - `data`: The stream data to be checked.
    ///
    /// # Returns
    ///
    /// A boolean indicating whether the stream data is supported.
    fn supports_stream(&self, data: StreamData) -> bool;

    /// Reallocates resources based on the given stream data.
    ///
    /// # Arguments
    ///
    /// - `stream_data`: The new stream data for reallocation.
    fn reallocate(&mut self, stream_data: StreamData) {}

    /// Resets the module to its initial state.
    fn reset(&mut self) {}

    /// Calculates the latency for the module.
    ///
    /// # Arguments
    ///
    /// - `input_latencies`: An array representing latencies for each input.
    ///
    /// # Returns
    ///
    /// An array representing latencies for each output.
    fn latency(
        &self,
        input_latencies: EnumMapArray<Self::Inputs, f64>,
    ) -> EnumMapArray<Self::Outputs, f64>;

    /// Processes the module with the given context.
    ///
    /// # Arguments
    ///
    /// - `context`: The processing context for the module.
    ///
    /// # Returns
    ///
    /// The status of the process after execution.
    fn process(
        &mut self,
        stream_data: &StreamData,
        inputs: &[&[Self::Sample]],
        outputs: &mut [&mut [Self::Sample]],
    ) -> ProcessStatus;
}

impl<M: Module> RawModule for M {
    type Sample = M::Sample;

    #[inline]
    fn inputs(&self) -> usize {
        <M::Inputs as Enum>::Count::USIZE
    }

    #[inline]
    fn outputs(&self) -> usize {
        <M::Outputs as Enum>::Count::USIZE
    }

    #[inline]
    fn supports_stream(&self, data: StreamData) -> bool {
        M::supports_stream(self, data)
    }

    #[inline]
    fn reallocate(&mut self, stream_data: StreamData) {
        M::reallocate(self, stream_data)
    }

    #[inline]
    fn reset(&mut self) {
        M::reset(self)
    }

    fn process(
        &mut self,
        stream_data: &StreamData,
        inputs: &[&[Self::Sample]],
        outputs: &mut [&mut [Self::Sample]],
    ) -> ProcessStatus {
        M::process(self, stream_data, inputs, outputs)
    }
}

/// Trait representing a constructor for a module.
///
/// Module constructors are responsible for allocating a new module with the provided stream data,
/// and return modules in a usable state.
pub trait ModuleConstructor {
    /// The type of module that will be created.
    type Module: Module;

    /// Allocates a new module with the provided stream data.
    ///
    /// # Arguments
    ///
    /// * `stream_data` - The data related to the stream configuration and metadata.
    ///
    /// # Returns
    ///
    /// A newly allocated module.
    fn allocate(&self, stream_data: StreamData) -> Self::Module;
}

impl<'a, M: ModuleConstructor> ModuleConstructor for &'a M {
    type Module = M::Module;

    #[inline]
    fn allocate(&self, stream_data: StreamData) -> Self::Module {
        M::allocate(self, stream_data)
    }
}

/// A struct that clones a module of type `M`.
pub struct ModuleCloner<M> {
    module: M,
}

impl<M: Module + Clone> ModuleConstructor for ModuleCloner<M> {
    type Module = M;

    #[inline]
    fn allocate(&self, _: StreamData) -> Self::Module {
        self.module.clone()
    }
}
