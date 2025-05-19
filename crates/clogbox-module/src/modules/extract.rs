//! Module to extract an audio stream into a ring buffer to be used elsewhere (i.e., a GUI).
use crate::context::ProcessContext;
use crate::{Module, PrepareResult, ProcessResult, Samplerate};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{Empty, Enum, Mono};

struct Producer<T, Audio: Enum = Mono> {
    producer: fixed_ringbuf::Producer<EnumMapArray<Audio, T>>,
}

impl<T, Audio: Enum> Producer<T, Audio> {
    fn send_buffer(&self, buffer: EnumMapArray<Audio, &[T]>) -> usize
    where
        T: Copy,
    {
        let Some(len) = buffer.values().map(|slice| slice.len()).min() else {
            return 0;
        };
        for i in 0..len {
            self.producer.push_overriding(EnumMapArray::new(|e| buffer[e][i]));
        }
        len
    }
}

/// Module which extracts the input signal into a ring buffer. This module needs to be "armed" by providing the
/// producer side of the ring buffer (a [`fixed_ringbuf::Producer`]) to activate this module.
///
/// This module does not do pass-through; you'll need to separately route the input signal somewhere else to also
/// hear it.
pub struct ExtractAudio<T, Audio: Enum = Mono> {
    tx: Option<Producer<T, Audio>>,
}

impl<T, Audio: Enum> ExtractAudio<T, Audio> {
    /// Constant default value of this module
    pub const CONST_DEFAULT: Self = Self { tx: None };
}

impl<T, Audio: Enum> Default for ExtractAudio<T, Audio> {
    fn default() -> Self {
        Self::CONST_DEFAULT
    }
}

impl<T, Audio: Enum> ExtractAudio<T, Audio> {
    /// Connect the ring buffer to this module.
    ///
    /// This instance of the module will then send all incoming audio to the provided ring buffer, which will then be
    /// accessible through the corresponding [`fixed_ringbuf::Consumer`] instance.
    ///
    /// # Arguments
    ///
    /// * `producer`: Ring buffer producer instance to use with this module
    pub fn connect(&mut self, producer: fixed_ringbuf::Producer<EnumMapArray<Audio, T>>) {
        self.tx = Some(Producer { producer });
    }
}

impl<T: Send + Copy + Default, Audio: Enum> Module for ExtractAudio<T, Audio> {
    type Sample = T;
    type AudioIn = Audio;
    type AudioOut = Empty;
    type ParamsIn = Empty;
    type ParamsOut = Empty;
    type NoteIn = Empty;
    type NoteOut = Empty;

    fn prepare(&mut self, _sample_rate: Samplerate, _block_size: usize) -> PrepareResult {
        PrepareResult { latency: 0.0 }
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        let Some(tx) = &mut self.tx else {
            return ProcessResult { tail: None };
        };

        let buffer = EnumMapArray::new(|e| &context.audio_in[e]);
        tx.send_buffer(buffer);

        ProcessResult { tail: None }
    }
}
