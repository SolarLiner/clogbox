use crate::context::ProcessContext;
use crate::{Module, PrepareResult, ProcessResult, Samplerate};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::typenum::Unsigned;
use clogbox_enum::{enum_iter, Empty, Enum, Mono};
use ringbuf::traits::RingBuffer;
use ringbuf::{HeapRb, StaticRb};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, Copy, Clone)]
pub enum BufferSize {
    Samples(usize),
    Seconds(f64),
}

impl BufferSize {
    pub fn as_samples(&self, samplerate: Samplerate) -> usize {
        match self {
            Self::Samples(s) => *s,
            Self::Seconds(s) => (*s * samplerate.value()).round() as usize,
        }
    }

    pub const fn as_samples_direct(&self) -> Option<usize> {
        match self {
            Self::Samples(s) => Some(*s),
            Self::Seconds(_) => None,
        }
    }
}

pub struct CircularBuffer<T, Audio: Enum = Mono, Ringbuf: 'static + Send + RingBuffer<Item = T> = HeapRb<T>> {
    __sample: PhantomData<T>,
    tx: EnumMapArray<Audio, Arc<Mutex<Ringbuf>>>,
}

impl<T, Audio: Enum, Ringbuf: 'static + Send + RingBuffer<Item = T>> Clone for CircularBuffer<T, Audio, Ringbuf> {
    fn clone(&self) -> Self {
        Self {
            __sample: PhantomData,
            tx: self.tx.clone(),
        }
    }
}

impl<T: Send + Default, Audio: Enum> CircularBuffer<T, Audio, HeapRb<T>> {
    pub fn new(capacity: usize) -> Self {
        Self {
            __sample: PhantomData,
            tx: EnumMapArray::new(|_| Arc::new(Mutex::new(HeapRb::new(capacity)))),
        }
    }
}

impl<T: Send, Audio: Enum, const N: usize> CircularBuffer<T, Audio, StaticRb<T, N>> {
    pub fn new(data: EnumMapArray<Audio, [T; N]>) -> Self {
        Self {
            __sample: PhantomData,
            tx: data.map(|_, array| Arc::new(Mutex::new(StaticRb::from(array)))),
        }
    }
}

impl<T: Send + Default, Audio: Enum, const N: usize> Default for CircularBuffer<T, Audio, StaticRb<T, N>> {
    fn default() -> Self {
        Self::new(EnumMapArray::new(|_| std::array::from_fn(|_| T::default())))
    }
}

impl<T: 'static + Send + Copy, Audio: Enum, Ringbuf: 'static + Send + RingBuffer<Item = T>>
    CircularBuffer<T, Audio, Ringbuf>
{
    pub fn iter_frames(&self) -> impl '_ + Iterator<Item = EnumMapArray<Audio, T>> {
        IterFrames::new(EnumMapArray::new(|e| self.tx[e].lock().unwrap()))
    }
}

impl<T: Copy, Audio: Enum, Ringbuf: 'static + Send + RingBuffer<Item = T>> CircularBuffer<T, Audio, Ringbuf> {
    pub fn send_frame(&self, frame: EnumMapArray<Audio, T>) {
        for (tx, x) in self.tx.values().zip(frame.values().copied()) {
            tx.lock().unwrap().push_overwrite(x);
        }
    }

    pub fn send_buffer(&self, buffer: EnumMapArray<Audio, &[T]>)
    where
        T: Copy,
    {
        for (tx, slice) in self.tx.values().zip(buffer.values()) {
            tx.lock().unwrap().push_slice_overwrite(slice);
        }
    }

    pub fn send_buffer_interleaved(&mut self, buffer: &[T]) -> bool
    where
        T: Copy,
    {
        if buffer.len() % Audio::Count::USIZE != 0 {
            return false;
        }

        for chunk in buffer.chunks(Audio::Count::USIZE) {
            self.send_frame(EnumMapArray::from_iter(chunk.iter().copied()))
        }

        true
    }
}

impl<T: Copy + Default, Audio: Enum, Ringbuf: 'static + Send + RingBuffer<Item = T>> CircularBuffer<T, Audio, Ringbuf> {
    pub fn read_frame(&self) -> Option<EnumMapArray<Audio, T>> {
        let opt_map = EnumMapArray::new(|e| self.tx[e].lock().unwrap().try_pop());
        if opt_map.values().all(|x| x.is_some()) {
            Some(opt_map.map(|_, opt| opt.unwrap()))
        } else {
            None
        }
    }

    pub fn read_buffer(&self, buffer: EnumMapArray<Audio, &mut [T]>) -> usize
    where
        Audio::Count: typenum::IsGreaterOrEqual<typenum::U1>,
    {
        let mut rb = EnumMapArray::new(|e| self.tx[e].lock().unwrap());
        let capacity = rb
            .values()
            .map(|tx| tx.occupied_len())
            .min()
            .unwrap()
            .min(Audio::Count::USIZE);
        for (e, slice) in buffer {
            let actual = rb[e].pop_slice(&mut slice[..capacity]);
            // Should never trigger, because we have already checked that the ring buffers have enough occupancy, and
            // they're all locked at once (so can't be modified by another thread).
            assert_eq!(actual, capacity);
        }
        capacity
    }

    pub fn read_buffer_interleaved(&mut self, buffer: &mut [T]) -> usize
    where
        Audio::Count: typenum::IsGreaterOrEqual<typenum::U1>,
    {
        let mut rb = EnumMapArray::new(|e| self.tx[e].lock().unwrap());
        let capacity = rb.values().map(|tx| tx.occupied_len()).min().unwrap();
        let slice_len = capacity * Audio::Count::USIZE;
        for (i, s) in buffer[..slice_len].iter_mut().enumerate() {
            let e = Audio::from_usize(i % Audio::Count::USIZE);
            *s = rb[e].try_pop().unwrap();
        }
        capacity
    }
}

struct IterFrames<'a, T: 'static + Send, Audio: Enum, Ringbuf: 'static + Send + RingBuffer<Item = T>> {
    __sample: PhantomData<T>,
    rb: EnumMapArray<Audio, MutexGuard<'a, Ringbuf>>,
    i: usize,
}

impl<'a, T: 'static + Send + Copy, Audio: Enum, Ringbuf: 'static + Send + RingBuffer<Item = T>> Iterator
    for IterFrames<'a, T, Audio, Ringbuf>
{
    type Item = EnumMapArray<Audio, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rb.iter().any(|(_, tx)| tx.occupied_len() <= self.i) {
            return None;
        }

        let result = EnumMapArray::new(|e| {
            let (left, right) = self.rb[e].as_slices();
            if self.i < left.len() {
                left[self.i]
            } else {
                right[self.i - left.len()]
            }
        });

        self.i += 1;
        Some(result)
    }
}

impl<'a, T: 'static + Send, Audio: Enum, Ringbuf: 'static + Send + RingBuffer<Item = T>>
    IterFrames<'a, T, Audio, Ringbuf>
{
    fn new(rb: EnumMapArray<Audio, MutexGuard<'a, Ringbuf>>) -> Self {
        Self {
            __sample: PhantomData,
            rb,
            i: 0,
        }
    }
}

pub struct ExtractAudio<T: 'static + Send, Audio: Enum = Mono> {
    buffer_size: BufferSize,
    buffer: Option<CircularBuffer<T, Audio>>,
}

impl<T: 'static + Send, Audio: Enum> ExtractAudio<T, Audio> {
    pub fn circular_buffer(&self) -> Option<CircularBuffer<T, Audio>> {
        self.buffer.clone()
    }
}

impl<T: 'static + Send, Audio: Enum> ExtractAudio<T, Audio> {
    pub fn new(buffer_size: BufferSize) -> Self {
        Self {
            buffer_size,
            buffer: None,
        }
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

    fn prepare(&mut self, sample_rate: Samplerate, _block_size: usize) -> PrepareResult {
        self.buffer = Some(CircularBuffer::<_, _>::new(self.buffer_size.as_samples(sample_rate)));
        PrepareResult { latency: 0.0 }
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        let Some(cb) = &mut self.buffer else {
            return ProcessResult { tail: None };
        };

        let buffer = EnumMapArray::new(|e| &context.audio_in[e]);
        cb.send_buffer(buffer);

        ProcessResult { tail: None }
    }
}
