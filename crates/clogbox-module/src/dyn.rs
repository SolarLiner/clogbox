//! # Dynamic module implementations
//!
//! This module provides dynamic dispatch wrappers for modules, allowing for runtime
//! polymorphism in audio processing graphs.

use crate::context::{ProcessContext, StreamContext};
use crate::{Module, NoteSlice, ParamSlice, PrepareResult, ProcessResult, Samplerate};
use clogbox_enum::{count, Enum};
use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops;

/// Context for dynamic audio processing that provides access to audio, parameter, and note data.
///
/// This struct is used in dynamic dispatch scenarios where the exact types of the
/// audio processing modules are not known at compile time. It contains references
/// to input and output buffers for audio samples, parameters, and MIDI notes.
pub struct DynProcessContext<'a, T> {
    /// Input audio buffer references, indexed by channel number
    pub audio_in: &'a dyn ops::Index<usize, Output = [T]>,
    /// Output audio buffer references, indexed by channel number
    pub audio_out: &'a mut dyn ops::IndexMut<usize, Output = [T]>,
    /// Input parameter buffer references, indexed by parameter ID
    pub params_in: &'a dyn ops::Index<usize, Output = ParamSlice>,
    /// Output parameter buffer references, indexed by parameter ID
    pub params_out: &'a mut dyn ops::IndexMut<usize, Output = ParamSlice>,
    /// Input MIDI note buffer references, indexed by note channel
    pub note_in: &'a dyn ops::Index<usize, Output = NoteSlice>,
    /// Output MIDI note buffer references, indexed by note channel
    pub note_out: &'a mut dyn ops::IndexMut<usize, Output = NoteSlice>,
    /// Current stream processing context containing timing information
    pub stream_context: &'a StreamContext,
}

/// Represents the type of socket/connection in an audio processing module.
///
/// This enum is used to categorize connections between modules in an audio processing graph.
/// It distinguishes between audio signals, control parameters, and MIDI notes.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum SocketType {
    /// Represents audio signal connections, typically containing sample data.
    Audio,
    /// Represents parameter connections for automation and control values.
    Params,
    /// Represents MIDI note connections for musical event data.
    Note,
}

/// Represents a connection point in an audio processing module.
///
/// A Socket is an input or output port through which audio data, parameters, or MIDI notes
/// can flow between modules in an audio processing graph. Each socket has a name and a type
/// that defines what kind of data it handles.
pub struct Socket<'a> {
    /// The human-readable name of the socket, useful for display in UIs and debugging.
    pub name: Cow<'a, str>,
    /// The type of data this socket handles (audio, parameters, or notes).
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

/// Maps enum-based indexing to usize-based indexing for immutable data access.
///
/// This struct adapts a data structure that can be indexed by `usize` to allow
/// indexing by enum variants. It provides a bridge between dynamic usize-based
/// indexing (used in the dynamic dispatch context) and the static enum-based
/// indexing typically used in the statically-typed module API.
struct EnumIndexMapping<'a, T: ?Sized, E> {
    /// Phantom data to track the enum type at compile-time
    __enum: PhantomData<E>,
    /// The underlying data indexed by usize
    data: &'a dyn ops::Index<usize, Output = T>,
}

impl<'a, T: ?Sized, E> EnumIndexMapping<'a, T, E> {
    /// Creates a new mapping from an usize-indexable data structure.
    ///
    /// This constructor takes a reference to any type that implements
    /// [`Index<usize>`](ops::Index) and wraps it to provide enum-based indexing.
    fn new(data: &'a dyn ops::Index<usize, Output = T>) -> Self {
        Self {
            __enum: PhantomData,
            data,
        }
    }
}

impl<T: ?Sized, E: Enum> ops::Index<E> for EnumIndexMapping<'_, T, E> {
    type Output = T;

    /// Implements indexing by enum variant.
    ///
    /// This method converts the enum variant to an usize using the [`Enum`] trait
    /// and then uses that to index into the underlying data.
    fn index(&self, index: E) -> &Self::Output {
        self.data.index(index.to_usize())
    }
}

/// Maps enum-based indexing to usize-based indexing for mutable data access.
///
/// Similar to [`EnumIndexMapping`], but provides mutable access to the underlying data.
/// This struct adapts a data structure that can be mutably indexed by `usize` to allow
/// mutable indexing by enum variants, bridging between dynamic and static typing approaches.
struct EnumIndexMutMapping<'a, T: ?Sized, E> {
    /// Phantom data to track the enum type at compile-time
    __enum: PhantomData<E>,
    /// The underlying mutable data indexed by usize
    data: &'a mut dyn ops::IndexMut<usize, Output = T>,
}

impl<'a, T: ?Sized, E> EnumIndexMutMapping<'a, T, E> {
    /// Creates a new mapping from a mutable usize-indexable data structure.
    ///
    /// This constructor takes a mutable reference to any type that implements
    /// [`IndexMut<usize>`](ops::IndexMut) and wraps it to provide enum-based mutable indexing.
    fn new(data: &'a mut dyn ops::IndexMut<usize, Output = T>) -> Self {
        Self {
            __enum: PhantomData,
            data,
        }
    }
}

impl<T: ?Sized, E: Enum> ops::Index<E> for EnumIndexMutMapping<'_, T, E> {
    type Output = T;

    /// Implements immutable indexing by enum variant.
    ///
    /// This method converts the enum variant to a usize using the [`Enum`] trait
    /// and then uses that to immutably index into the underlying data.
    fn index(&self, index: E) -> &Self::Output {
        self.data.index(index.to_usize())
    }
}

impl<T: ?Sized, E: Enum> ops::IndexMut<E> for EnumIndexMutMapping<'_, T, E> {
    /// Implements mutable indexing by enum variant.
    ///
    /// This method converts the enum variant to a usize using the `Enum` trait
    /// and then uses that to mutably index into the underlying data, allowing
    /// the caller to modify the indexed value.
    fn index_mut(&mut self, index: E) -> &mut Self::Output {
        self.data.index_mut(index.to_usize())
    }
}

/// Trait for dynamic dispatch audio processing modules.
///
/// This trait enables runtime polymorphism for audio modules by providing a common
/// interface for querying module connections, preparing for processing, and
/// performing the actual audio processing. It allows modules with different static types
/// to be treated uniformly at runtime.
///
/// The type parameter `T` represents the sample type (typically f32 or f64).
pub trait DynModule<T> {
    /// Returns the number of input sockets of the specified type.
    ///
    /// This method allows querying how many input connections of a given type
    /// (audio, parameters, or notes) the module supports.
    fn num_inputs(&self, socket_type: SocketType) -> usize;

    /// Returns information about a specific input socket.
    ///
    /// Given a socket type and index, this method returns a Socket structure
    /// containing the name and type of the specified input connection.
    fn input_socket(&self, socket_type: SocketType, index: usize) -> Socket<'_>;

    /// Returns the number of output sockets of the specified type.
    ///
    /// This method allows querying how many output connections of a given type
    /// (audio, parameters, or notes) the module provides.
    fn num_outputs(&self, socket_type: SocketType) -> usize;

    /// Returns information about a specific output socket.
    ///
    /// Given a socket type and index, this method returns a Socket structure
    /// containing the name and type of the specified output connection.
    fn output_socket(&self, socket_type: SocketType, index: usize) -> Socket<'_>;

    /// Prepares the module for processing.
    ///
    /// This method is called before processing begins, allowing the module to
    /// initialize internal state based on the sample rate and block size.
    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult;

    /// Processes audio, parameter, and note data.
    ///
    /// This is the core method where the actual audio processing takes place.
    /// It receives a context containing all input and output buffers, processes
    /// the inputs, and writes the results to the outputs.
    fn process(&mut self, context: DynProcessContext<T>) -> ProcessResult;
}
