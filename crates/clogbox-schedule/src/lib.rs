use crate::event::EventBuffer;
use crate::fixed_delay::{AudioDelay, EventDelay};
use crate::graph::ConnectionTypeMismatch;
use crate::module::{ExecutionContext, ProcessStatus, RawModule, SocketType, StreamData};
use crate::note::{NoteEvent, NoteKey};
use crate::storage::{MappedStorage, Storage};
use crate::sum::BufferOverflow;
use clogbox_enum::enum_map::EnumMapArray;
use derive_more::{Deref, DerefMut};
use num_traits::{Float, NumAssign, Zero};
use smallvec::SmallVec;
use storage::SharedStorage;

pub mod event;
mod fixed_delay;
mod graph;
pub mod module;
pub mod note;
pub mod param;
pub mod storage;
mod sum;

/// Type alias for parameter events (i.e. at "control rate").
pub type ParamBuffer = EventBuffer<f32>;
/// Type alias for note events.
pub type NoteBuffer = EventBuffer<(NoteKey, NoteEvent)>;

/// Wrapper for timestamped values.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Deref, DerefMut)]
pub struct Timestamped<T> {
    /// Relative sample position to the point at which the process method is invoked.
    pub sample: usize,
    /// Inner value
    #[deref]
    #[deref_mut]
    pub value: T,
}

impl<T> Timestamped<T> {
    /// Copies the timestamp, but returns a reference to the value.
    pub fn as_ref(&self) -> Timestamped<&T> {
        Timestamped {
            sample: self.sample,
            value: &self.value,
        }
    }

    /// Copies the timestamp, but returns a mutable reference to the value.
    pub fn as_mut(&mut self) -> Timestamped<&mut T> {
        Timestamped {
            sample: self.sample,
            value: &mut self.value,
        }
    }

    /// Maps the timestamped value, keeping the timestamp.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Timestamped<U> {
        Timestamped {
            sample: self.sample,
            value: f(self.value),
        }
    }
}

pub struct ScheduledModule<T> {
    pub clear_inputs: EnumMapArray<SocketType, SmallVec<[bool; 16]>>,
    pub inputs: EnumMapArray<SocketType, SmallVec<[usize; 16]>>,
    pub outputs: EnumMapArray<SocketType, SmallVec<[usize; 16]>>,
    pub module: Box<dyn RawModule<Scalar = T>>,
}

impl<T: Zero> ScheduledModule<T> {
    fn process_module(
        &mut self,
        stream_data: &StreamData,
        audio_storage: &dyn SharedStorage<Value = [T]>,
        param_storage: &dyn SharedStorage<Value = ParamBuffer>,
        note_storage: &dyn SharedStorage<Value = NoteBuffer>,
    ) -> ProcessStatus {
        for i in self.inputs[SocketType::Audio]
            .clone()
            .into_iter()
            .filter(|i| self.clear_inputs[SocketType::Audio][*i])
        {
            let mut buf = audio_storage.get_mut(i);
            buf.fill_with(T::zero);
        }
        for i in self.inputs[SocketType::Param]
            .clone()
            .into_iter()
            .filter(|i| self.clear_inputs[SocketType::Param][*i])
        {
            let mut buf = param_storage.get_mut(i);
            buf.clear();
        }
        for i in self.inputs[SocketType::Note]
            .clone()
            .into_iter()
            .filter(|i| self.clear_inputs[SocketType::Note][*i])
        {
            let mut buf = note_storage.get_mut(i);
            buf.clear();
        }
        let audio_storage = MappedStorage {
            storage: audio_storage,
            index_map: &self.inputs[SocketType::Audio],
        };
        let param_storage = MappedStorage {
            storage: param_storage,
            index_map: &self.inputs[SocketType::Param],
        };
        let note_storage = MappedStorage {
            storage: note_storage,
            index_map: &self.inputs[SocketType::Note],
        };
        let ctx = ExecutionContext {
            audio_storage: &audio_storage,
            param_storage: &param_storage,
            note_storage: &note_storage,
            stream_data,
        };
        self.module.process(&ctx)
    }
}

pub enum ScheduledItem<T> {
    Module(ScheduledModule<T>),
    AudioDelay {
        input: usize,
        output: usize,
        delay: AudioDelay<T>,
    },
    ParamDelay {
        input: usize,
        output: usize,
        delay: EventDelay<f32>,
    },
    NoteDelay {
        input: usize,
        output: usize,
        delay: EventDelay<(NoteKey, NoteEvent)>,
    },
    Sum {
        typ: SocketType,
        inputs: SmallVec<[usize; 16]>,
        output: usize,
    },
}

pub struct Schedule<T> {
    audio_storage: Storage<[T]>,
    param_storage: Storage<ParamBuffer>,
    note_storage: Storage<NoteBuffer>,
    input_buffer_indices: SmallVec<[usize; 16]>,
    output_buffer_indices: SmallVec<[usize; 16]>,
    items: Box<[ScheduledItem<T>]>,
}

impl<T: Float + NumAssign + az::Cast<usize>> Schedule<T> {
    pub fn process(&mut self, stream_data: &StreamData, inputs: &[&[T]], outputs: &mut [&mut [T]]) -> ProcessStatus {
        let size = stream_data.buffer_size;

        // Copy inputs
        for i in self.input_buffer_indices.iter().copied() {
            self.audio_storage.get_mut(i)[..size].copy_from_slice(inputs[i]);
        }

        // Process items in order
        for item in &mut self.items[..] {
            match item {
                ScheduledItem::Module(module) => {
                    module.process_module(
                        stream_data,
                        &self.audio_storage,
                        &self.param_storage,
                        &self.note_storage,
                    );
                }
                ScheduledItem::AudioDelay { input, output, delay } => {
                    delay.process_buffer(*self.audio_storage.get(*input), *self.audio_storage.get_mut(*output));
                }
                ScheduledItem::ParamDelay { input, output, delay } => {
                    match delay.process_buffer(
                        size,
                        *self.param_storage.get(*input),
                        *self.param_storage.get_mut(*output),
                    ) {
                        Ok(()) => {}
                        Err(_) => eprintln!("ERROR: Buffer overflow in param delay {} -> {}", *input, *output),
                    }
                }
                ScheduledItem::NoteDelay { input, output, delay } => {
                    match delay.process_buffer(
                        size,
                        *self.note_storage.get(*input),
                        *self.note_storage.get_mut(*output),
                    ) {
                        Ok(()) => {}
                        Err(_) => eprintln!("ERROR: Buffer overflow in param delay {} -> {}", *input, *output),
                    }
                }
                ScheduledItem::Sum { typ, inputs, output } => match typ {
                    SocketType::Audio => {
                        sum::audio(
                            MappedStorage {
                                storage: &self.audio_storage,
                                index_map: &**inputs,
                            },
                            *self.audio_storage.get_mut(*output),
                        );
                    }
                    SocketType::Param => {
                        match sum::events(
                            MappedStorage {
                                storage: &self.param_storage,
                                index_map: &**inputs,
                            },
                            *self.param_storage.get_mut(*output),
                        ) {
                            Ok(()) => {}
                            Err(_) => {
                                eprintln!("ERROR: Buffer overflow in param sum {} -> {}", *output, *output)
                            }
                        }
                    }
                    SocketType::Note => {
                        match sum::events(
                            MappedStorage {
                                storage: &self.note_storage,
                                index_map: &**inputs,
                            },
                            *self.note_storage.get_mut(*output),
                        ) {
                            Ok(()) => {}
                            Err(_) => {
                                eprintln!("ERROR: Buffer overflow in param sum {} -> {}", *output, *output)
                            }
                        }
                    }
                },
            }
        }

        ProcessStatus::Continue
    }
}

pub struct ScheduleSerialized<T> {
    pub num_audio: usize,
    pub num_params: usize,
    pub num_note: usize,
    pub input_buffer_indices: Vec<usize>,
    pub output_buffer_indices: Vec<usize>,
    // TODO: Proper serialization
    pub items: Vec<ScheduledItem<T>>,
}

impl<T: Zero> ScheduleSerialized<T> {
    pub fn construct(self, buffer_size: usize) -> Schedule<T> {
        let audio_storage = Storage::new(self.num_audio, |_| {
            std::iter::repeat_with(T::zero).take(buffer_size).collect()
        });
        let param_storage = Storage::new(self.num_params, |_| Box::new(ParamBuffer::new(buffer_size)));
        let note_storage = Storage::new(self.num_note, |_| Box::new(NoteBuffer::new(buffer_size)));
        Schedule {
            audio_storage,
            param_storage,
            note_storage,
            input_buffer_indices: self.input_buffer_indices.into(),
            output_buffer_indices: self.output_buffer_indices.into(),
            items: self.items.into_boxed_slice(),
        }
    }
}
