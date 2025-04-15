use clogbox_enum::{count, Enum};
use std::marker::PhantomData;
use std::ops;
use std::borrow::Cow;
use crate::{Module, NoteSlice, ParamSlice, PrepareResult, ProcessContext, ProcessResult, Samplerate, StreamContext};

pub struct DynProcessContext<'a, T> {
    pub audio_in: &'a dyn ops::Index<usize, Output = [T]>,
    pub audio_out: &'a mut dyn ops::IndexMut<usize, Output = [T]>,
    pub params_in: &'a dyn ops::Index<usize, Output = ParamSlice>,
    pub params_out: &'a mut dyn ops::IndexMut<usize, Output = ParamSlice>,
    pub note_in: &'a dyn ops::Index<usize, Output = NoteSlice>,
    pub note_out: &'a mut dyn ops::IndexMut<usize, Output = NoteSlice>,
    pub stream_context: &'a StreamContext,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum SocketType {
    Audio,
    Params,
    Note,
}

pub struct Socket<'a> {
    pub name: Cow<'a, str>,
    pub socket_type: SocketType,
}

impl<M: Module> DynModule<M::Sample> for M {
    fn num_inputs(&self, socket_type: SocketType) -> usize {
        match socket_type {
            SocketType::Audio => count::<M::AudioIn>(),
            SocketType::Params => count::<M::ParamsIn>(),
            SocketType::Note => count::<M::NoteIn>(),
        }
    }

    fn input_socket(&self, socket_type: SocketType, index: usize) -> Socket<'_> {
        match socket_type {
            SocketType::Audio => {
                let e = M::AudioIn::from_usize(index);
                Socket {
                    name: e.name().to_string().into(),
                    socket_type,
                }
            }
            SocketType::Params => {
                let e = M::ParamsIn::from_usize(index);
                Socket {
                    name: e.name().to_string().into(),
                    socket_type,
                }
            }
            SocketType::Note => {
                let e = M::NoteIn::from_usize(index);
                Socket {
                    name: e.name().to_string().into(),
                    socket_type,
                }
            }
        }
    }

    fn num_outputs(&self, socket_type: SocketType) -> usize {
        match socket_type {
            SocketType::Audio => count::<M::AudioIn>(),
            SocketType::Params => count::<M::ParamsIn>(),
            SocketType::Note => count::<M::NoteIn>(),
        }
    }

    fn output_socket(&self, socket_type: SocketType, index: usize) -> Socket<'_> {
        match socket_type {
            SocketType::Audio => {
                let e = M::AudioOut::from_usize(index);
                Socket {
                    name: e.name().to_string().into(),
                    socket_type,
                }
            }
            SocketType::Params => {
                let e = M::ParamsOut::from_usize(index);
                Socket {
                    name: e.name().to_string().into(),
                    socket_type,
                }
            }
            SocketType::Note => {
                let e = M::NoteOut::from_usize(index);
                Socket {
                    name: e.name().to_string().into(),
                    socket_type,
                }
            }
        }
    }

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
        <M as Module>::prepare(self, sample_rate, block_size)
    }

    fn process(&mut self, context: DynProcessContext<M::Sample>) -> ProcessResult {
        let audio_in = EnumIndexMapping::new(context.audio_in);
        let mut audio_out = EnumIndexMutMapping::new(context.audio_out);
        let params_in = EnumIndexMapping::new(context.params_in);
        let mut params_out = EnumIndexMutMapping::new(context.params_out);
        let note_in = EnumIndexMapping::new(context.note_in);
        let mut note_out = EnumIndexMutMapping::new(context.note_out);
        let context = ProcessContext {
            audio_in: &audio_in,
            audio_out: &mut audio_out,
            params_in: &params_in,
            params_out: &mut params_out,
            note_in: &note_in,
            note_out: &mut note_out,
            stream_context: context.stream_context,
            __phantom: PhantomData,
        };
        <M as Module>::process(self, context)
    }
}

struct EnumIndexMapping<'a, T: ?Sized, E> {
    __enum: PhantomData<E>,
    data: &'a dyn ops::Index<usize, Output=T>,
}

impl<'a, T: ?Sized, E> EnumIndexMapping<'a, T, E> {
    fn new(data: &'a dyn ops::Index<usize, Output=T>) -> Self {
        Self { __enum: PhantomData, data }
    }
}

impl<T: ?Sized, E: Enum> ops::Index<E> for EnumIndexMapping<'_, T, E> {
    type Output = T;

    fn index(&self, index: E) -> &Self::Output {
        self.data.index(index.to_usize())
    }
}

struct EnumIndexMutMapping<'a, T: ?Sized, E> {
    __enum: PhantomData<E>,
    data: &'a mut dyn ops::IndexMut<usize, Output=T>,
}

impl<'a, T: ?Sized, E> EnumIndexMutMapping<'a, T, E> {
    fn new(data: &'a mut dyn ops::IndexMut<usize, Output=T>) -> Self {
        Self { __enum: PhantomData, data }
    }
}

impl<T: ?Sized, E: Enum> ops::Index<E> for EnumIndexMutMapping<'_, T, E> {
    type Output = T;

    fn index(&self, index: E) -> &Self::Output {
        self.data.index(index.to_usize())
    }
}

impl<T: ?Sized, E: Enum> ops::IndexMut<E> for EnumIndexMutMapping<'_, T, E> {
    fn index_mut(&mut self, index: E) -> &mut Self::Output {
        self.data.index_mut(index.to_usize())
    }
}

pub trait DynModule<T> {
    fn num_inputs(&self, socket_type: SocketType) -> usize;
    fn input_socket(&self, socket_type: SocketType, index: usize) -> Socket<'_>;
    fn num_outputs(&self, socket_type: SocketType) -> usize;
    fn output_socket(&self, socket_type: SocketType, index: usize) -> Socket<'_>;
    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult;
    fn process(&mut self, context: DynProcessContext<T>) -> ProcessResult;
}