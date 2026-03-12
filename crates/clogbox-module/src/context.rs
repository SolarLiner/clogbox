//! # Context types for audio processing
//!
//! This module provides context structures that are passed to modules during processing,
//! containing information about the current processing state.

use crate::eventbuffer::{self, Timestamped, TimestampedCollection, TimestampedCollectionMut};
use crate::note::NoteEvent;
use crate::{Module, Samplerate};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{Empty, Enum};
use num_traits::Zero;
use std::marker::PhantomData;
use std::ops;

/// Unified event handle, enabling the use of a single event buffer for all events
#[derive(Debug, Copy, Clone)]
pub enum UnifiedEvent<Param, Note> {
    /// Parameter event (single value representing the natural value of the parameter)
    Parameter(Param, f32),
    /// Note event
    Note(Note, NoteEvent),
}

impl<Param: Copy + PartialEq, Note: Copy + PartialEq> UnifiedEvent<Param, Note> {
    pub fn is_param(&self, param: Param) -> bool {
        matches!(self, &Self::Parameter(p, _) if p == param)
    }

    pub fn is_note(&self, note: Note) -> bool {
        matches!(self, &Self::Note(n, _) if n == note)
    }
}

pub type EventBuffer<Param, Note> = eventbuffer::EventBuffer<UnifiedEvent<Param, Note>>;

pub type EventSlice<Param, Note> = eventbuffer::EventSlice<UnifiedEvent<Param, Note>>;

pub const DEFAULT_EVENT_BUFFER_CAPACITY: usize = 2048;

pub fn filter_events<P1: Copy, N1: Copy, P2, N2>(
    filter_params: impl Fn(P1) -> Option<P2>,
    filter_notes: impl Fn(N1) -> Option<N2>,
) -> impl Fn(&mut EventBuffer<P2, N2>, &EventSlice<P1, N1>) {
    move |tgt, src| {
        for event in src {
            let Some(Timestamped { timestamp, data }) = event.filter_map(|data| match data {
                UnifiedEvent::Parameter(param, value) => Some(UnifiedEvent::Parameter(filter_params(param)?, value)),
                UnifiedEvent::Note(note, value) => Some(UnifiedEvent::Note(filter_notes(note)?, value)),
            }) else {
                continue;
            };
            tgt.push(timestamp, data);
        }
    }
}

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
    /// Reference to the input events
    pub events_in: &'a dyn TimestampedCollection<UnifiedEvent<M::ParamsIn, M::NoteIn>>,
    /// Mutable reference to the output events
    pub events_out: &'a mut dyn TimestampedCollectionMut<UnifiedEvent<M::ParamsOut, M::NoteOut>>,
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
    /// Storage for the input events
    pub events_in: EventBuffer<M::ParamsIn, M::NoteIn>,
    /// Storage for the output events
    pub events_out: EventBuffer<M::ParamsOut, M::NoteOut>,
    __phantom: PhantomData<M>,
}

impl<M: ?Sized + Module> Default for OwnedProcessContext<M> {
    fn default() -> Self {
        Self {
            audio_in: AudioStorage::new(|_| vec![].into_boxed_slice()),
            audio_out: AudioStorage::new(|_| vec![].into_boxed_slice()),
            events_in: EventBuffer::new(DEFAULT_EVENT_BUFFER_CAPACITY),
            events_out: EventBuffer::new(DEFAULT_EVENT_BUFFER_CAPACITY),
            __phantom: PhantomData,
        }
    }
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
            events_in: EventBuffer::new(event_capacity),
            events_out: EventBuffer::new(event_capacity),
            __phantom: PhantomData,
        }
    }

    pub fn resize_audio_buffers(&mut self, new_capacity: usize)
    where
        M::Sample: Copy + Zero,
    {
        self.audio_in.resize(new_capacity);
        self.audio_out.resize(new_capacity);
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
            events_in: &self.events_in,
            events_out: &mut self.events_out,
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

    pub fn resize(&mut self, new_capacity: usize)
    where
        T: Copy + Zero,
    {
        for box_ in self.storage.values_mut() {
            let overlap = box_.len().min(new_capacity);
            let mut new_box = Box::from_iter(std::iter::repeat_with(T::zero).take(new_capacity));
            new_box[..overlap].copy_from_slice(&box_[..overlap]);
            *box_ = new_box;
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

impl<T> AudioStorage<Empty, T> {
    /// Empty buffer containing no elements.
    pub const EMPTY: Self = Self { storage: EnumMapArray::CONST_DEFAULT };
}

/// Storage for events associated with enum channels.
#[derive(Debug)]
pub struct EventStorage<E: Enum, T> {
    /// The internal storage mapping each enum variant to an event buffer.
    storage: EnumMapArray<E, eventbuffer::EventBuffer<T>>,
}

impl<E: Enum, T> ops::Deref for EventStorage<E, T> {
    type Target = EnumMapArray<E, eventbuffer::EventBuffer<T>>;

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
    type Output = eventbuffer::EventBuffer<T>;

    /// Access event data for a given channel.
    fn index(&self, index: E) -> &Self::Output {
        &self.storage[index]
    }
}

impl<E: Enum, T> ops::IndexMut<E> for EventStorage<E, T> {
    /// Mutable access to event data for a given channel.
    fn index_mut(&mut self, index: E) -> &mut Self::Output {
        &mut self.storage[index]
    }
}

impl<E: Enum, T> EventStorage<E, T> {
    /// Creates event storage with pre-allocated capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            storage: EnumMapArray::new(|_| eventbuffer::EventBuffer::new(capacity)),
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
