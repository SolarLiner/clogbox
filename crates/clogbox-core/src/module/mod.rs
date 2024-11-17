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
//! use std::sync::Arc;
//! use az::CastFrom;
//! use typenum::U1;
//!
//! use clogbox_core::module::{enum_mapped_storage, BufferStorage, Module, ModuleContext, OwnedBufferStorage, ProcessStatus, StreamData};
//! use clogbox_core::param::{Params, EMPTY_PARAMS};
//! use clogbox_core::r#enum::{enum_iter, seq, Empty, Enum, Sequential};
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
//!     type Params = Empty;
//!
//!     fn get_params(&self) -> Arc<impl '_ + Params<Params=Self::Params>> {
//!         Arc::new(EMPTY_PARAMS)
//!     }
//!
//!     fn supports_stream(&self, data: StreamData) -> bool {
//!         true
//!     }
//!
//!     fn latency(&self, input_latency: EnumMapArray<Self::Inputs, f64>) -> EnumMapArray<Self::Outputs, f64> {
//!         input_latency
//!     }
//!
//!     fn process<S: BufferStorage<Sample=Self::Sample, Input=Self::Inputs, Output=Self::Outputs>> (&mut self, context: &mut ModuleContext<S>) -> ProcessStatus {
//!         for inp in enum_iter::<In>() {
//!             let (inp_buf, out_buf) = context.get_input_output_pair(inp, inp);
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
//! let stream_data = StreamData { sample_rate: 44100.0 ,bpm: 120., block_size };
//! let mut storage = OwnedBufferStorage::new(1, 1, block_size);
//! storage.inputs[0].iter_mut().enumerate().for_each(|(i, x)| *x = i as f32);
//! let mut context = ModuleContext {
//!     stream_data,
//!     buffers: enum_mapped_storage(&mut storage),
//! };
//! my_module.process(&mut context);
//! drop(context);
//! assert_eq!(-4., storage.outputs[0][4]);
//! ```
pub mod analysis;
pub mod sample;
pub mod utilitarian;

use crate::param::{Params, RawParams};
use crate::r#enum::enum_map::EnumMapArray;
use crate::r#enum::{count, Either, Enum};
use az::Cast;
use num_traits::Zero;
use std::marker::PhantomData;
use std::ops;
use std::sync::Arc;
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

    /// Returns the number of params of the module.
    fn get_params(&self) -> Arc<dyn '_ + RawParams>;

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
        context: &mut ModuleContext<
            &mut dyn BufferStorage<Sample = Self::Sample, Input = usize, Output = usize>,
        >,
    ) -> ProcessStatus;
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

    type Params: Enum;

    fn get_params(&self) -> Arc<impl '_ + Params<Params=Self::Params>>;

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
    fn process<
        S: BufferStorage<Sample = Self::Sample, Input = Self::Inputs, Output = Self::Outputs>,
    >(
        &mut self,
        context: &mut ModuleContext<S>,
    ) -> ProcessStatus;
}

impl<M: Module<Sample: Zero>> RawModule for M {
    type Sample = M::Sample;

    #[inline]
    fn inputs(&self) -> usize {
        <M::Inputs as Enum>::Count::USIZE
    }

    #[inline]
    fn outputs(&self) -> usize {
        <M::Outputs as Enum>::Count::USIZE
    }

    fn get_params(&self) -> Arc<dyn '_ + RawParams> {
        M::get_params(self)
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

    fn process<'o, 'i>(
        &mut self,
        context: &mut ModuleContext<
            &mut dyn BufferStorage<Sample = Self::Sample, Input = usize, Output = usize>,
        >,
    ) -> ProcessStatus {
        let storage = MappedBufferStorage {
            storage: &mut *context.buffers,
            mapper: |x: Either<M::Inputs, M::Outputs>| match x {
                Either::Left(a) => a.cast(),
                Either::Right(b) => b.cast(),
            },
            __io_types: PhantomData,
        };
        M::process(
            self,
            &mut ModuleContext {
                stream_data: context.stream_data,
                buffers: storage,
            },
        )
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

pub trait BufferStorage {
    type Sample;
    type Input;
    type Output;

    fn get_input_buffer(&self, input: Self::Input) -> &[Self::Sample];
    fn get_output_buffer(&mut self, output: Self::Output) -> &mut [Self::Sample];
    fn get_inout_pair(
        &mut self,
        input: Self::Input,
        output: Self::Output,
    ) -> (&[Self::Sample], &mut [Self::Sample]);

    fn reset(&mut self);
    fn clear_input(&mut self, input: Self::Input);
    fn clear_output(&mut self, output: Self::Output);
}

impl<T: Zero, S: ops::DerefMut<Target = [T]>> BufferStorage for [S] {
    type Sample = T;
    type Input = usize;
    type Output = usize;

    fn get_input_buffer(&self, input: usize) -> &[T] {
        &*self[input]
    }

    fn get_output_buffer(&mut self, output: usize) -> &mut [T] {
        &mut *self[output]
    }

    fn get_inout_pair(&mut self, input: usize, output: usize) -> (&[T], &mut [T]) {
        assert_ne!(input, output, "Cannot alias same buffer");
        let inp = &*self[input];
        // Safety: No aliasing created with assert above, and self[output] is already exclusive
        // because self is exclusive at this point
        let out = unsafe {
            let len = self[output].len();
            std::slice::from_raw_parts_mut(self[output].as_ptr().cast_mut(), len)
        };
        (inp, out)
    }

    fn reset(&mut self) {
        for slice in self.iter_mut() {
            slice.fill_with(T::zero);
        }
    }

    fn clear_input(&mut self, input: Self::Input) {
        self[input].fill_with(T::zero);
    }

    fn clear_output(&mut self, output: Self::Output) {
        self[output].fill_with(T::zero);
    }
}

pub struct RawModuleStorage<'outer, 'inner, T> {
    inputs: &'outer [&'inner [T]],
    outputs: &'outer mut [&'inner mut [T]],
}

impl<'outer, 'inner, T: Zero> BufferStorage for RawModuleStorage<'outer, 'inner, T> {
    type Sample = T;
    type Input = usize;
    type Output = usize;

    fn get_input_buffer(&self, input: usize) -> &[T] {
        &*self.inputs[input]
    }

    fn get_output_buffer(&mut self, output: usize) -> &mut [T] {
        &mut *self.outputs[output]
    }

    fn get_inout_pair(&mut self, input: usize, output: usize) -> (&[T], &mut [T]) {
        (&*self.inputs[input], &mut *self.outputs[output])
    }

    fn reset(&mut self) {
        for slice in self.outputs.iter_mut() {
            slice.fill_with(T::zero);
        }
    }

    fn clear_input(&mut self, _input: Self::Input) {
        // Not supported
    }

    fn clear_output(&mut self, output: Self::Output) {
        self.outputs[output].fill_with(T::zero);
    }
}

#[derive(Debug, Clone)]
pub struct OwnedBufferStorage<T> {
    pub inputs: Vec<Box<[T]>>,
    pub outputs: Vec<Box<[T]>>,
}

impl<T: Zero> OwnedBufferStorage<T> {
    pub fn new(num_inputs: usize, num_outputs: usize, block_size: usize) -> Self {
        Self {
            inputs: Vec::from_iter(
                std::iter::repeat_with(|| {
                    std::iter::repeat_with(T::zero).take(block_size).collect()
                })
                .take(num_inputs),
            ),
            outputs: Vec::from_iter(
                std::iter::repeat_with(|| {
                    std::iter::repeat_with(T::zero).take(block_size).collect()
                })
                .take(num_outputs),
            ),
        }
    }
}

impl<T: Zero> BufferStorage for OwnedBufferStorage<T> {
    type Sample = T;
    type Input = usize;
    type Output = usize;

    fn get_input_buffer(&self, input: Self::Input) -> &[Self::Sample] {
        &self.inputs[input]
    }

    fn get_output_buffer(&mut self, output: Self::Output) -> &mut [Self::Sample] {
        &mut self.outputs[output]
    }

    fn get_inout_pair(
        &mut self,
        input: Self::Input,
        output: Self::Output,
    ) -> (&[Self::Sample], &mut [Self::Sample]) {
        (&*self.inputs[input], &mut *self.outputs[output])
    }

    fn reset(&mut self) {
        for slice in self.inputs.iter_mut().chain(self.outputs.iter_mut()) {
            slice.fill_with(T::zero);
        }
    }

    fn clear_input(&mut self, input: Self::Input) {
        self.inputs[input].fill_with(T::zero);
    }

    fn clear_output(&mut self, output: Self::Output) {
        self.outputs[output].fill_with(T::zero);
    }
}

#[derive(Debug, Clone)]
pub struct MappedBufferStorage<S, In, Out, F> {
    pub storage: S,
    pub mapper: F,
    pub __io_types: PhantomData<(In, Out)>,
}

impl<In, Out, S: BufferStorage<Input = usize, Output = usize>, F: Fn(Either<In, Out>) -> usize>
    BufferStorage for MappedBufferStorage<S, In, Out, F>
{
    type Sample = S::Sample;
    type Input = In;
    type Output = Out;
    fn get_input_buffer(&self, input: In) -> &[Self::Sample] {
        let ix = (self.mapper)(Either::Left(input));
        self.storage.get_input_buffer(ix)
    }

    fn get_output_buffer(&mut self, output: Out) -> &mut [Self::Sample] {
        let ix = (self.mapper)(Either::Right(output));
        self.storage.get_output_buffer(ix)
    }

    fn get_inout_pair(&mut self, input: In, output: Out) -> (&[Self::Sample], &mut [Self::Sample]) {
        let input = (self.mapper)(Either::Left(input));
        let output = (self.mapper)(Either::Right(output));
        self.storage.get_inout_pair(input, output)
    }

    fn reset(&mut self) {
        self.storage.reset()
    }

    fn clear_input(&mut self, input: Self::Input) {
        self.storage.clear_input((self.mapper)(Either::Left(input)))
    }

    fn clear_output(&mut self, output: Self::Output) {
        self.storage
            .clear_output((self.mapper)(Either::Right(output)))
    }
}

impl<'a, S: ?Sized + BufferStorage> BufferStorage for &'a mut S {
    type Sample = S::Sample;
    type Input = S::Input;
    type Output = S::Output;

    fn get_input_buffer(&self, input: Self::Input) -> &[Self::Sample] {
        S::get_input_buffer(self, input)
    }

    fn get_output_buffer(&mut self, output: Self::Output) -> &mut [Self::Sample] {
        S::get_output_buffer(self, output)
    }

    fn get_inout_pair(
        &mut self,
        input: Self::Input,
        output: Self::Output,
    ) -> (&[Self::Sample], &mut [Self::Sample]) {
        S::get_inout_pair(self, input, output)
    }

    fn reset(&mut self) {
        S::reset(self)
    }

    fn clear_input(&mut self, input: Self::Input) {
        S::clear_input(self, input)
    }

    fn clear_output(&mut self, output: Self::Output) {
        S::clear_output(self, output)
    }
}

pub const fn enum_mapped_storage<S: BufferStorage<Input=usize, Output=usize>, In: Enum, Out: Enum>(storage: S) -> impl BufferStorage<Sample=S::Sample, Input=In, Output=Out> {
    MappedBufferStorage {
        storage,
        mapper: |x: Either<In, Out>| match x {
            Either::Left(input) => input.cast(),
            Either::Right(output) => output.cast(),
        },
        __io_types: PhantomData,
    }
}

pub struct ModuleContext<S> {
    pub stream_data: StreamData,
    pub buffers: S,
}

impl<S> ModuleContext<S> {
    pub fn with_io<S2>(&mut self, buffer_storage: S2) -> ModuleContext<S2> {
        ModuleContext {
            stream_data: self.stream_data,
            buffers: buffer_storage,
        }
    }

    pub fn stream_data(&self) -> &StreamData {
        &self.stream_data
    }
}

impl<S: BufferStorage> ModuleContext<S> {
    pub fn get_input(&self, input: S::Input) -> &[S::Sample] {
        self.buffers.get_input_buffer(input)
    }

    pub fn get_output(&mut self, output: S::Output) -> &mut [S::Sample] {
        self.buffers.get_output_buffer(output)
    }

    pub fn get_input_output_pair(
        &mut self,
        input: S::Input,
        output: S::Output,
    ) -> (&[S::Sample], &mut [S::Sample]) {
        self.buffers.get_inout_pair(input, output)
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
