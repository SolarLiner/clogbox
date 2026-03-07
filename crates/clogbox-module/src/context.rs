//! # Context types for audio processing
//!
//! This module provides context structures that are passed to modules during processing,
//! containing information about the current processing state.

use crate::eventbuffer::{EventBuffer, EventSlice};
use crate::note::NoteEvent;
use crate::{Module, NoteSlice, ParamSlice, Samplerate};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{Empty, Enum};
use num_traits::Zero;
use std::marker::PhantomData;
use std::ops;

/// Provides information about the audio stream, such as sample rate and block size.
#[derive(Debug, Copy, Clone)]
pub struct StreamContext {
    /// The sample rate of the audio stream.
    pub sample_rate: Samplerate,
    /// The size of each processing block.
    pub block_size: usize,
}

/// Contains all relevant data and state during a processing cycle for a module.
pub struct ProcessContext<'a, M: ?Sized + Module> {
    /// Reference to the input audio buffer for each input channel.
    pub audio_in: &'a dyn ops::Index<M::AudioIn, Output = [M::Sample]>,
    /// Mutable reference to the output audio buffer for each output channel.
    pub audio_out: &'a mut dyn ops::IndexMut<M::AudioOut, Output = [M::Sample]>,
    /// Reference to the input parameter values.
    pub params_in: &'a dyn ops::Index<M::ParamsIn, Output = ParamSlice>,
    /// Mutable reference to the output parameter values.
    pub params_out: &'a mut dyn ops::IndexMut<M::ParamsOut, Output = ParamSlice>,
    /// Reference to the current notes' input.
    pub note_in: &'a dyn ops::Index<M::NoteIn, Output = NoteSlice>,
    /// Mutable reference to the notes' output.
    pub note_out: &'a mut dyn ops::IndexMut<M::NoteOut, Output = NoteSlice>,
    /// The current stream context, containing info like sample rate and block size.
    pub stream_context: &'a StreamContext,
    /// Phantom data to associate the context with the module type.
    pub __phantom: PhantomData<&'a M>,
}

/// Contains owned, possibly more convenient, storage for process data for a module.
pub struct OwnedProcessContext<M: ?Sized + Module> {
    /// Storage for input audio data.
    pub audio_in: AudioStorage<M::AudioIn, M::Sample>,
    /// Storage for output audio data.
    pub audio_out: AudioStorage<M::AudioOut, M::Sample>,
    /// Storage for input parameters.
    pub params_in: EventStorage<M::ParamsIn, f32>,
    /// Storage for output parameters.
    pub params_out: EventStorage<M::ParamsOut, f32>,
    /// Storage for input notes.
    pub note_in: EventStorage<M::NoteIn, NoteEvent>,
    /// Storage for output notes.
    pub note_out: EventStorage<M::NoteOut, NoteEvent>,
    /// Phantom data for type association.
    __phantom: PhantomData<M>,
}

impl<M: ?Sized + Module> OwnedProcessContext<M> {
    /// Creates a new owned process context with default storage sizes.
    ///
    /// # Parameters
    ///
    /// * `block_size` - The size of processing blocks.
    /// * `event_capacity` - Capacity for events like notes and parameters.
    ///
    /// # Returns
    ///
    /// A new `OwnedProcessContext` with zeroed audio and allocated event buffers.
    pub fn new(block_size: usize, event_capacity: usize) -> Self
    where
        M::Sample: Zero,
    {
        Self {
            audio_in: AudioStorage::zeroed(block_size),
            audio_out: AudioStorage::zeroed(block_size),
            params_in: EventStorage::with_capacity(event_capacity),
            params_out: EventStorage::with_capacity(event_capacity),
            note_in: EventStorage::with_capacity(event_capacity),
            note_out: EventStorage::with_capacity(event_capacity),
            __phantom: PhantomData,
        }
    }

    /// Executes a processing closure within this context.
    ///
    /// # Parameters
    ///
    /// * `stream_context` - Reference to the current stream context.
    /// * `func` - Closure to execute with a `ProcessContext` argument.
    ///
    /// # Returns
    ///
    /// The result of the closure execution.
    pub fn process_with<R>(&mut self, stream_context: &StreamContext, func: impl FnOnce(ProcessContext<M>) -> R) -> R {
        func(ProcessContext {
            audio_in: &self.audio_in,
            audio_out: &mut self.audio_out,
            params_in: &self.params_in,
            params_out: &mut self.params_out,
            note_in: &self.note_in,
            note_out: &mut self.note_out,
            stream_context,
            __phantom: PhantomData,
        })
    }
}

/// Storage for audio data associated with enum channels.
#[derive(Debug, Clone)]
pub struct AudioStorage<E: Enum, T> {
    /// The internal storage mapping each enum variant to a buffer.
    storage: EnumMapArray<E, Box<[T]>>,
}

impl<E: Enum, T> ops::Deref for AudioStorage<E, T> {
    type Target = EnumMapArray<E, Box<[T]>>;

    /// Dereferences to the internal storage.
    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl<E: Enum, T> ops::DerefMut for AudioStorage<E, T> {
    /// Mutable dereference to the internal storage.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

impl<E: Enum, T> AudioStorage<E, T> {
    /// Creates a new `AudioStorage` with buffers filled by the provided initializer.
    pub fn new(fill: impl Fn(E) -> Box<[T]>) -> Self {
        Self {
            storage: EnumMapArray::new(fill),
        }
    }

    /// Creates an `AudioStorage` with buffers initialized with default values.
    pub fn default(capacity: usize) -> Self
    where
        T: Default,
    {
        Self::new(|_| Box::from_iter(std::iter::repeat_with(T::default).take(capacity)))
    }

    /// Creates an `AudioStorage` with buffers filled with zeroes.
    pub fn zeroed(capacity: usize) -> Self
    where
        T: Zero,
    {
        Self::new(|_| Box::from_iter(std::iter::repeat_with(T::zero).take(capacity)))
    }

    /// Copies input data into the storage.
    ///
    /// # Parameters
    ///
    /// * `input` - Input source implementing Index, providing slices for each channel.
    pub fn copy_from_input<I: ?Sized + ops::Index<E, Output = [T]>>(&mut self, input: &I)
    where
        T: Copy,
    {
        for (e, slice) in self.storage.iter_mut() {
            slice.copy_from_slice(&input.index(e)[..slice.len()]);
        }
    }

    /// Copies stored data into the output.
    ///
    /// # Parameters
    ///
    /// * `output` - Output destination implementing IndexMut, accepting slices for each channel.
    pub fn copy_to_output<O: ?Sized + ops::IndexMut<E, Output = [T]>>(&self, output: &mut O)
    where
        T: Copy,
    {
        for (e, slice) in self.storage.iter() {
            output.index_mut(e).copy_from_slice(&slice[..slice.len()]);
        }
    }
}

impl<E: Enum, T> ops::Index<E> for AudioStorage<E, T> {
    type Output = [T];

    /// Access audio data for a given channel.
    fn index(&self, index: E) -> &Self::Output {
        &self.storage[index][..]
    }
}

impl<E: Enum, T> ops::IndexMut<E> for AudioStorage<E, T> {
    /// Mutable access to audio data for a given channel.
    fn index_mut(&mut self, index: E) -> &mut Self::Output {
        &mut self.storage[index][..]
    }
}

/// Storage for events associated with enum channels.
#[derive(Debug, Clone)]
pub struct EventStorage<E: Enum, T> {
    /// The internal storage mapping each enum variant to an event buffer.
    storage: EnumMapArray<E, EventBuffer<T>>,
}

impl<E: Enum, T> ops::Deref for EventStorage<E, T> {
    type Target = EnumMapArray<E, EventBuffer<T>>;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl<E: Enum, T> ops::DerefMut for EventStorage<E, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

impl<E: Enum, T> ops::Index<E> for EventStorage<E, T> {
    type Output = EventSlice<T>;

    /// Access event data for a given channel.
    fn index(&self, index: E) -> &Self::Output {
        self.storage[index].as_slice()
    }
}

impl<E: Enum, T> ops::IndexMut<E> for EventStorage<E, T> {
    /// Mutable access to event data for a given channel.
    fn index_mut(&mut self, index: E) -> &mut Self::Output {
        self.storage[index].as_mut_slice()
    }
}

impl<E: Enum, T> Default for EventStorage<E, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Enum, T> EventStorage<E, T> {
    /// Creates a new empty event storage.
    pub fn new() -> Self {
        Self {
            storage: EnumMapArray::new(|_| EventBuffer::new()),
        }
    }

    /// Creates event storage with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            storage: EnumMapArray::new(|_| EventBuffer::with_capacity(capacity)),
        }
    }
}

impl<T> EventStorage<Empty, T> {
    /// Creates an empty event storage.
    pub const fn empty() -> Self {
        Self {
            storage: EnumMapArray::CONST_DEFAULT,
        }
    }
}
