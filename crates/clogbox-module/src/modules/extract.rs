use crate::context::ProcessContext;
use crate::{Module, PrepareResult, ProcessResult, Samplerate};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{Empty, Enum, Mono};

struct Producer<T, Audio: Enum = Mono> {
    tx: fixed_ringbuf::Producer<EnumMapArray<Audio, T>>,
}

impl<T, Audio: Enum> Producer<T, Audio> {
    pub fn send_frame(&self, frame: EnumMapArray<Audio, T>) {
        self.tx.push_overriding(frame);
    }

    pub fn send_buffer(&self, buffer: EnumMapArray<Audio, &[T]>) -> usize
    where
        T: Copy,
    {
        let Some(len) = buffer.values().map(|slice| slice.len()).min() else {
            return 0;
        };
        for i in 0..len {
            self.tx.push_overriding(EnumMapArray::new(|e| buffer[e][i]));
        }
        len
    }
}

pub struct ExtractAudio<T, Audio: Enum = Mono> {
    tx: Option<Producer<T, Audio>>,
}

impl<T, Audio: Enum> ExtractAudio<T, Audio> {
    pub const CONST_NEW: Self = Self { tx: None };
}

impl<T, Audio: Enum> Default for ExtractAudio<T, Audio> {
    fn default() -> Self {
        Self::CONST_NEW
    }
}

impl<T, Audio: Enum> ExtractAudio<T, Audio> {
    pub fn set_tx(&mut self, tx: fixed_ringbuf::Producer<EnumMapArray<Audio, T>>) {
        self.tx = Some(Producer { tx });
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
