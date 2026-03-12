use crate::clock;
use crate::clock::Clock;
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{enum_iter, Empty, Enum};
use clogbox_filters::saturators::SimpleSaturator;
use clogbox_module::context::{OwnedProcessContext, ProcessContext, UnifiedEvent};
use clogbox_module::eventbuffer::{TimestampedCollection, TimestampedCollectionMut};
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
use fixed_ringbuf::RingBufferStatic;
use num_traits::{Float, Zero};
use numeric_literals::replace_float_literals;
use std::num::NonZeroU32;
use clogbox_filters::Saturator;

pub struct BucketBrigade<T, Channels: Enum, const SIZE: usize = 512> {
    clock_context: OwnedProcessContext<Clock>,
    clock: Clock,
    delay: RingBufferStatic<EnumMapArray<Channels, T>, SIZE>,
    current: EnumMapArray<Channels, T>,
    saturators: SimpleSaturator<T>,
}

impl<T: Zero, Channels: Enum, const SIZE: usize> BucketBrigade<T, Channels, SIZE> {
    pub fn new(sample_rate: f32, block_size: usize, frequency: f32, jitter: f32) -> Self {
        Self {
            clock_context: OwnedProcessContext::new(block_size, block_size),
            clock: Clock::new(sample_rate, frequency, jitter),
            delay: RingBufferStatic::new(),
            current: EnumMapArray::new(|_| T::zero()),
            saturators: SimpleSaturator::new(|x| x),
        }
    }

    pub fn with_saturator(mut self, saturator: SimpleSaturator<T>) -> Self {
        self.saturators = saturator;
        self
    }
}

impl<T: Send + Float, Channels: Enum, const SIZE: usize> Module for BucketBrigade<T, Channels, SIZE> {
    type Sample = T;
    type AudioIn = Channels;
    type AudioOut = Channels;
    type ParamsIn = clock::Params;
    type ParamsOut = Empty;
    type NoteIn = Empty;
    type NoteOut = Empty;

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
        self.clock_context.resize_audio_buffers(block_size);
        self.clock.prepare(sample_rate, block_size);
        self.delay.clear();
        self.current = EnumMapArray::new(|_| T::zero());
        PrepareResult { latency: SIZE as _ }
    }

    #[replace_float_literals(T::from(literal).unwrap())]
    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        self.clock_context.events_in.clear();
        self.clock_context.events_out.clear();
        for event in context.events_in.slice() {
            self.clock_context.events_in.push(event.timestamp, event.data);
        }
        self.clock_context
            .process_with(context.stream_context, |ctx| self.clock.process(ctx));

        for (range, events) in self
            .clock_context
            .events_out
            .slice()
            .chunk_events(context.stream_context.block_size)
        {
            for event in events {
                let UnifiedEvent::Parameter(clock::ParamsOut::Tick, value) = event.data else {
                    continue;
                };
                for _ in 0..value as usize {
                    if self.delay.is_full() {
                        self.current = self.delay.pop().unwrap_or_else(|| EnumMapArray::new(|_| T::zero()));
                        self.current.values_mut().for_each(|s| *s = self.saturators.saturate(*s));
                    }
                    debug_assert!(self
                        .delay
                        .push(EnumMapArray::new(|ch| context.audio_in[ch][event.timestamp]))
                        .is_ok());
                }
            }

            for i in range {
                for ch in enum_iter::<Channels>() {
                    context.audio_out[ch][i] = self.current[ch];
                }
            }
        }
        ProcessResult {
            tail: NonZeroU32::new(SIZE as _),
        }
    }
}
