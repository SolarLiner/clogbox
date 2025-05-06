use crate::eventbuffer::{EventBuffer, EventSlice};
use crate::note::NoteEvent;
use crate::{Module, NoteSlice, ParamSlice, Samplerate};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{Empty, Enum};
use num_traits::Zero;
use std::marker::PhantomData;
use std::ops;

#[derive(Debug, Copy, Clone)]
pub struct StreamContext {
    pub sample_rate: Samplerate,
    pub block_size: usize,
}

pub struct ProcessContext<'a, M: ?Sized + Module> {
    pub audio_in: &'a dyn ops::Index<M::AudioIn, Output = [M::Sample]>,
    pub audio_out: &'a mut dyn ops::IndexMut<M::AudioOut, Output = [M::Sample]>,
    pub params_in: &'a dyn ops::Index<M::ParamsIn, Output = ParamSlice>,
    pub params_out: &'a mut dyn ops::IndexMut<M::ParamsOut, Output = ParamSlice>,
    pub note_in: &'a dyn ops::Index<M::NoteIn, Output = NoteSlice>,
    pub note_out: &'a mut dyn ops::IndexMut<M::NoteOut, Output = NoteSlice>,
    pub stream_context: &'a StreamContext,
    pub __phantom: PhantomData<&'a M>,
}

pub struct OwnedProcessContext<M: ?Sized + Module> {
    pub audio_in: AudioStorage<M::AudioIn, M::Sample>,
    pub audio_out: AudioStorage<M::AudioOut, M::Sample>,
    pub params_in: EventStorage<M::ParamsIn, f32>,
    pub params_out: EventStorage<M::ParamsOut, f32>,
    pub note_in: EventStorage<M::NoteIn, NoteEvent>,
    pub note_out: EventStorage<M::NoteOut, NoteEvent>,
    __phantom: PhantomData<M>,
}

impl<M: ?Sized + Module> OwnedProcessContext<M> {
    pub fn new(block_size: usize, event_capacity: usize) -> Self
    where
        M::Sample: Zero,
    {
        let zero = M::Sample::zero;
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

#[derive(Debug, Clone)]
pub struct AudioStorage<E: Enum, T> {
    storage: EnumMapArray<E, Box<[T]>>,
}

impl<E: Enum, T> ops::Deref for AudioStorage<E, T> {
    type Target = EnumMapArray<E, Box<[T]>>;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl<E: Enum, T> ops::DerefMut for AudioStorage<E, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

impl<E: Enum, T> AudioStorage<E, T> {
    pub fn new(fill: impl Fn(E) -> Box<[T]>) -> Self {
        Self {
            storage: EnumMapArray::new(fill),
        }
    }

    pub fn default(capacity: usize) -> Self
    where
        T: Default,
    {
        Self::new(|_| Box::from_iter(std::iter::repeat_with(T::default).take(capacity)))
    }

    pub fn zeroed(capacity: usize) -> Self
    where
        T: Zero,
    {
        Self::new(|_| Box::from_iter(std::iter::repeat_with(T::zero).take(capacity)))
    }

    pub fn copy_from_input<I: ?Sized + ops::Index<E, Output = [T]>>(&mut self, input: &I)
    where
        T: Copy,
    {
        for (e, slice) in self.storage.iter_mut() {
            slice.copy_from_slice(&input.index(e)[..slice.len()]);
        }
    }
    
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

    fn index(&self, index: E) -> &Self::Output {
        &self.storage[index][..]
    }
}

impl<E: Enum, T> ops::IndexMut<E> for AudioStorage<E, T> {
    fn index_mut(&mut self, index: E) -> &mut Self::Output {
        &mut self.storage[index][..]
    }
}

#[derive(Debug, Clone)]
pub struct EventStorage<E: Enum, T> {
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

    fn index(&self, index: E) -> &Self::Output {
        self.storage[index].as_slice()
    }
}

impl<E: Enum, T> ops::IndexMut<E> for EventStorage<E, T> {
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
    pub fn new() -> Self {
        Self {
            storage: EnumMapArray::new(|_| EventBuffer::new()),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            storage: EnumMapArray::new(|_| EventBuffer::with_capacity(capacity)),
        }
    }
}

impl<T> EventStorage<Empty, T> {
    pub const fn empty() -> Self {
        Self {
            storage: EnumMapArray::CONST_DEFAULT,
        }
    }
}
