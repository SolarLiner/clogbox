use az::CastFrom;
use clogbox_enum::{Empty, Enum, Mono};
use clogbox_math::interpolation::{BoundaryCondition, Interpolation, Linear};
use clogbox_module::context::{AudioStorage, EventStorage, ProcessContext};
use clogbox_module::eventbuffer::Timestamped;
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
use num_traits::Zero;
use num_traits::{Float, Num};
use std::marker::PhantomData;
use std::ops;

pub struct Phasor<T> {
    sample_rate: T,
    frequency: T,
    step: T,
    current: T,
}

impl<T: Copy + Num> Phasor<T> {
    pub fn new(sample_rate: T, frequency: T) -> Self
    where
        T: Zero,
    {
        let step = frequency / sample_rate;
        Self {
            sample_rate,
            frequency,
            step,
            current: T::zero(),
        }
    }

    pub fn step(&self) -> T {
        self.step
    }

    pub fn set_sample_rate(&mut self, sample_rate: T) {
        self.sample_rate = sample_rate;
        self.step = self.frequency / self.sample_rate;
    }

    pub fn set_frequency(&mut self, frequency: T) {
        self.frequency = frequency;
        self.step = self.frequency / self.sample_rate;
    }
}

impl<T: CastFrom<f64> + Float> Phasor<T> {
    pub fn advance(&mut self, num_samples: usize) -> usize
    where
        T: CastFrom<f64>,
    {
        let end = self.current + self.step * T::cast_from(num_samples as f64);
        self.current = end.fract();
        end.floor().to_usize().unwrap()
    }

    pub fn process_sample(&mut self) -> (T, usize) {
        let current = self.current;
        (current, self.advance(1))
    }

    pub fn process_slice(&mut self, slice: &mut [T]) -> usize {
        let mut i = 0;
        for s in slice {
            let (current, advance) = self.process_sample();
            *s = current;
            i += advance;
        }
        i
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Enum)]
pub enum PhasorParams {
    Frequency,
    Reset,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Enum)]
pub enum PhasorEvents {
    Clock,
}

impl<T: CastFrom<f64> + Float> Module for Phasor<T> {
    type Sample = T;
    type AudioIn = Empty;
    type AudioOut = Mono;
    type ParamsIn = PhasorParams;
    type ParamsOut = PhasorEvents;
    type NoteIn = Empty;
    type NoteOut = Empty;

    fn prepare(&mut self, sample_rate: Samplerate, _block_size: usize) -> PrepareResult {
        self.set_sample_rate(T::cast_from(sample_rate.value()));
        PrepareResult { latency: 0.0 }
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        for i in 0..context.stream_context.block_size {
            if let Some(&Timestamped { data: value, .. }) = context.params_in[PhasorParams::Reset].at(i) {
                self.current = T::cast_from(value as _).fract();
            }
            if let Some(&Timestamped { data: value, .. }) = context.params_in[PhasorParams::Frequency].at(i) {
                self.set_frequency(T::cast_from(value as _));
            }

            let (next, ticks) = self.process_sample();
            context.audio_out[Mono][i] = next;
            for _ in 0..ticks {
                context.params_out[PhasorEvents::Clock].push(i, 0.0);
            }
        }
        ProcessResult { tail: None }
    }
}

pub struct Wavetable<T: CastFrom<f64> + Float, Interpolator = Linear> {
    phasor: Phasor<T>,
    phase_buffer: AudioStorage<Mono, T>,
    unused_phasor_resets: EventStorage<PhasorEvents, f32>,
    wavetable: Box<[T]>,
    interpolator: Interpolator,
}

impl<T: CastFrom<f64> + Float, Interpolator> ops::Deref for Wavetable<T, Interpolator> {
    type Target = Phasor<T>;

    fn deref(&self) -> &Self::Target {
        &self.phasor
    }
}

impl<T: CastFrom<f64> + Float, Interpolator> ops::DerefMut for Wavetable<T, Interpolator> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.phasor
    }
}

impl<T: CastFrom<f64> + Float, Interpolator: Interpolation<T>> Wavetable<T, Interpolator> {
    pub fn process_sample(&mut self) -> T {
        let phase = self.phasor.process_sample().0;
        let scale = T::cast_from(self.wavetable.len() as f64);
        self.interpolator
            .interpolate(BoundaryCondition::Wrap, &self.wavetable, scale * phase)
    }

    pub fn process_slice(&mut self, slice: &mut [T]) {
        self.phasor.process_slice(slice);
        let scale = T::cast_from(self.wavetable.len() as f64);
        for s in slice {
            *s = self
                .interpolator
                .interpolate(BoundaryCondition::Wrap, &self.wavetable, *s * scale);
        }
    }
}

impl<T: Zero + CastFrom<f64> + Float, Interpolator> Wavetable<T, Interpolator> {
    pub fn new(
        sample_rate: T,
        frequency: T,
        interpolator: Interpolator,
        wavetable: impl IntoIterator<Item = T>,
    ) -> Self {
        Self {
            phasor: Phasor::new(sample_rate, frequency),
            wavetable: Box::from_iter(wavetable),
            interpolator,
            phase_buffer: AudioStorage::zeroed(0),
            unused_phasor_resets: EventStorage::with_capacity(16),
        }
    }

    pub fn receive_wavetable(&mut self, wavetable: Box<[T]>) -> Box<[T]> {
        std::mem::replace(&mut self.wavetable, wavetable)
    }
}

impl<T: CastFrom<f64> + Float, Interpolator: Interpolation<T>> Module for Wavetable<T, Interpolator> {
    type Sample = T;
    type AudioIn = Empty;
    type AudioOut = Mono;
    type ParamsIn = PhasorParams;
    type ParamsOut = Empty;
    type NoteIn = Empty;
    type NoteOut = Empty;

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
        self.phasor.prepare(sample_rate, block_size);
        self.unused_phasor_resets = EventStorage::with_capacity(16);
        PrepareResult { latency: 0.0 }
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        let phasor_context = ProcessContext {
            audio_in: context.audio_in,
            audio_out: &mut self.phase_buffer,
            params_in: context.params_in,
            params_out: &mut self.unused_phasor_resets,
            note_in: context.note_in,
            note_out: context.note_out,
            stream_context: context.stream_context,
            __phantom: PhantomData,
        };
        self.phasor.process(phasor_context);

        self.process_slice(&mut context.audio_out[Mono]);
        ProcessResult { tail: None }
    }
}
