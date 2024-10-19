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
//! use clogbox_core::module::{StreamData, ProcessStatus, Module, ModuleContext, ModuleContextImpl, IoContext};
//! use clogbox_core::r#enum::{enum_iter, Enum, Sequential};
//! use clogbox_core::r#enum::enum_map::EnumMapArray;
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
//!     fn process(&mut self, context: &mut ModuleContext<Self>) -> ProcessStatus {
//!         for inp in enum_iter::<In>() {
//!             let (inp_buf, out_buf) = context.in_out(inp, inp);
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
//! let mut context = ModuleContextImpl {
//!     stream_data: &stream_data,
//!     io: IoContext {
//!         inputs: EnumMapArray::from_array(arr![&inputs]),
//!         outputs: EnumMapArray::from_array(arr![&mut outputs]),
//!     },
//! };
//! my_module.process(&mut context);
//! assert_eq!(-4., outputs[4]);
//! ```
pub mod analysis;
pub mod sample;
pub mod utilitarian;

use crate::r#enum::enum_map::EnumMapArray;
use crate::r#enum::Enum;
use std::ops;
use typenum::Unsigned;

/// A context that holds input and output buffers for processing I/O.
///
/// `IoContext` provides references to arrays of input and output data, which can be
/// used in various processing tasks.
#[derive(Debug)]
pub struct IoContext<'a, T, In: Enum, Out: Enum> {
    /// Represents the input data buffers.
    pub inputs: EnumMapArray<In, &'a [T]>,
    /// Represents the output data buffers.
    pub outputs: EnumMapArray<Out, &'a mut [T]>,
}

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

/// Represents the raw context for a module, which includes stream data, inputs, and outputs.
///
/// This struct is used internally within the module to handle audio processing operations.
///
/// # Type Parameters
///
/// * `T` - The type of the samples being processed.
#[derive(Debug)]
pub struct ModuleContextRaw<'a, T> {
    stream_data: &'a StreamData,
    inputs: &'a [&'a [T]],
    outputs: &'a mut [&'a mut [T]],
}

impl<'a, T> ModuleContextRaw<'a, T> {
    /// Returns the time duration of one sample in seconds by delegating to the stream data.
    pub fn dt(&self) -> f64 {
        self.stream_data.dt()
    }

    /// Forks the current module context into a new one with provided inputs and outputs.
    ///
    /// # Parameters
    ///
    /// * `inputs` - A slice of immutable slices representing the new inputs.
    /// * `outputs` - A slice of mutable slices representing the new outputs.
    ///
    /// # Returns
    ///
    /// Returns a `ModuleContextRaw` with the new inputs and outputs.
    pub fn fork<U>(
        &self,
        inputs: &'a [&'a [U]],
        outputs: &'a mut [&'a mut [U]],
    ) -> ModuleContextRaw<'a, U> {
        ModuleContextRaw {
            stream_data: self.stream_data,
            inputs,
            outputs,
        }
    }

    /// Returns a reference to the input slice at the specified index.
    ///
    /// # Parameters
    ///
    /// * `i` - The index of the input slice to retrieve.
    ///
    /// # Returns
    ///
    /// A reference to the input slice at the specified index.
    pub fn input_raw(&self, i: usize) -> &'a [T] {
        self.inputs[i]
    }

    /// Returns a mutable reference to the output buffer at the specified index.
    ///
    /// # Arguments
    ///
    /// * `i` - Index of the output buffer to be accessed.
    pub fn output_raw(&mut self, i: usize) -> &mut [T] {
        self.outputs[i]
    }

    /// Returns raw input and mutable output buffers.
    ///
    /// This method provides direct access to the raw input and output buffers for a given pair of indices,
    /// allowing low-level manipulation of the data. The input buffer is immutable while the output buffer is mutable.
    ///
    /// # Parameters
    /// - `i`: The index of the input buffer.
    /// - `j`: The index of the output buffer.
    ///
    /// # Returns
    /// A tuple containing:
    /// - A reference to the input buffer at index `i`.
    /// - A mutable reference to the output buffer at index `j`.
    pub fn in_out_raw(&mut self, i: usize, j: usize) -> (&[T], &mut [T]) {
        (self.inputs[i], self.outputs[j])
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
    fn process(&mut self, context: &mut ModuleContextRaw<Self::Sample>) -> ProcessStatus;
}

/// Type alias for `ModuleContextImpl`, making it simpler to use with modules by automatically
/// filling in the associated types from the `Module` trait.
pub type ModuleContext<'a, M> =
    ModuleContextImpl<'a, <M as Module>::Sample, <M as Module>::Inputs, <M as Module>::Outputs>;

/// Represents the context for a module, holding the stream data and I/O context.
pub struct ModuleContextImpl<'a, T, In: Enum, Out: Enum> {
    /// Reference to stream data.
    pub stream_data: &'a StreamData,
    /// The input and output context.
    pub io: IoContext<'a, T, In, Out>,
}

impl<'a, T, In: Enum, Out: Enum> ops::Deref for ModuleContextImpl<'a, T, In, Out> {
    type Target = IoContext<'a, T, In, Out>;

    fn deref(&self) -> &Self::Target {
        &self.io
    }
}

impl<'a, T, In: Enum, Out: Enum> ops::DerefMut for ModuleContextImpl<'a, T, In, Out> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.io
    }
}

impl<'a, T, In: Enum, Out: Enum> ModuleContextImpl<'a, T, In, Out> {
    /// Creates a `ModuleContextImpl` from a raw module context.
    ///
    /// This function ensures that the raw module context has the necessary
    /// number of inputs and outputs, then converts it to a `ModuleContextImpl`.
    ///
    /// # Arguments
    ///
    /// * `raw` - The raw module context to be converted.
    ///
    /// # Returns
    ///
    /// A `ModuleContextImpl` instance with initialized stream data and I/O context.
    ///
    /// # Safety
    ///
    /// This function assumes that the raw module context has valid lifetimes.
    /// The conversion process includes unsafe code to reborrow the mutable
    /// references in the raw outputs.
    pub fn from_raw(raw: ModuleContextRaw<'a, T>) -> Self {
        assert!(
            raw.inputs.len() >= In::Count::USIZE,
            "Not enough inputs in context for module"
        );
        assert!(
            raw.outputs.len() >= Out::Count::USIZE,
            "Not enough outputs in context for module"
        );
        Self {
            stream_data: raw.stream_data,
            io: IoContext {
                inputs: EnumMapArray::new(|k: In| raw.inputs[k.cast()]),
                outputs: EnumMapArray::new(|k: Out| {
                    let len = raw.outputs[k.cast()].len();
                    // Safety: lifetimes are preserved, and no alias is created as the raw module
                    // context is moved into this function.
                    // This performs a "mutable reborrow" which detaches the lifetimes from the incoming
                    // module context and attaches it to this one.
                    unsafe {
                        std::slice::from_raw_parts_mut(
                            raw.outputs[k.cast()].as_ptr().cast_mut(),
                            len,
                        )
                    }
                }),
            },
        }
    }

    /// Converts the current `ModuleContextImpl` to a `ModuleContextRaw`.
    ///
    /// This method creates a raw representation of the module context, which includes the
    /// stream data, inputs, and outputs.
    ///
    /// # Returns
    /// A `ModuleContextRaw` struct containing the stream data and references to input and
    /// mutable output slices.
    pub fn as_raw(&'a mut self) -> ModuleContextRaw<'a, T> {
        ModuleContextRaw {
            stream_data: self.stream_data,
            inputs: self.io.inputs.as_slice(),
            outputs: self.io.outputs.as_slice_mut(),
        }
    }

    /// Creates a new `ModuleContextImpl` by associating it with the provided I/O context.
    pub fn with_io_context<U, I2: Enum, O2: Enum>(
        &self,
        io_context: IoContext<'a, U, I2, O2>,
    ) -> ModuleContextImpl<'a, U, I2, O2> {
        ModuleContextImpl {
            stream_data: self.stream_data,
            io: io_context,
        }
    }

    /// Returns a reference to the input data corresponding to the given index.
    pub fn input(&self, i: In) -> &[T] {
        self.io.inputs[i]
    }

    /// Provides a mutable reference to the output buffer corresponding to the given index.
    pub fn output(&mut self, i: Out) -> &mut [T] {
        self.io.outputs[i]
    }

    /// Provides a mutable reference to the output buffer and an immutable reference to the input buffer
    /// for the specified indices.
    /// 
    /// Use this method when you need both an input and an output at the same time.
    ///
    /// # Parameters
    /// - `i`: The index of the input buffer.
    /// - `j`: The index of the output buffer.
    ///
    /// # Returns
    /// A tuple containing:
    /// - An immutable reference to the input buffer at the specified index.
    /// - A mutable reference to the output buffer at the specified index.
    pub fn in_out(&mut self, i: In, j: Out) -> (&[T], &mut [T]) {
        (self.inputs[i], self.outputs[j])
    }
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
    fn process(&mut self, context: &mut ModuleContext<Self>) -> ProcessStatus;
}

impl<M: Module> RawModule for M {
    type Sample = ();

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

    #[inline]
    fn process(&mut self, raw_context: &mut ModuleContextRaw<Self::Sample>) -> ProcessStatus {
        // Safety: ModuleCtxImpl<'a, Self::Sample, ...> is #[repr(transparent)]
        let context = unsafe {
            std::mem::transmute::<&mut ModuleContextRaw<Self::Sample>, &mut ModuleContext<M>>(
                raw_context,
            )
        };
        M::process(self, context)
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
