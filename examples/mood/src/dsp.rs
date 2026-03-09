use clogbox_clap::params::{frequency, linear, DynMapping, MappingExt, ParamId};
use clogbox_clap::{Plugin, PluginCreateContext, PluginDsp};
use clogbox_enum::{enum_iter, Empty, Enum, Stereo};
use clogbox_module::context::{OwnedProcessContext, ProcessContext, UnifiedEvent};
use clogbox_module::eventbuffer::TimestampedCollectionMut;
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
use mood::clock::Clock;
use std::fmt::Write;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum Params {
    Clock(mood::clock::Params),
}

impl ParamId for Params {
    fn text_to_value(&self, text: &str) -> Option<f32> {
        text.parse().ok()
    }

    fn default_value(&self) -> f32 {
        use mood::clock::Params::*;
        match self {
            Self::Clock(Frequency) => 8000.0,
            Self::Clock(Jitter) => 0.0,
        }
    }

    fn mapping(&self) -> DynMapping {
        use mood::clock::Params::*;
        match self {
            Self::Clock(Frequency) => frequency(20.0, 40000.0).into_dyn(),
            Self::Clock(Jitter) => linear(0.0, 1000.0).into_dyn(),
        }
    }

    fn value_to_text(&self, f: &mut dyn Write, denormalized: f32) -> std::fmt::Result {
        write!(f, "{denormalized:3.2} Hz")
    }
}

pub struct Dsp {
    clock_context: OwnedProcessContext<Clock>,
    clock: Clock,
    current_sample: [f32; 2],
}

impl Module for Dsp {
    type Sample = f32;
    type AudioIn = Stereo;
    type AudioOut = Stereo;
    type ParamsIn = Params;
    type ParamsOut = Empty;
    type NoteIn = Empty;
    type NoteOut = Empty;

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
        self.clock.prepare(sample_rate, block_size);
        PrepareResult { latency: 0.0 }
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        self.clock_context.events_in.clear();
        self.clock_context.events_out.clear();

        for event in context.events_in.slice() {
            let UnifiedEvent::Parameter(Params::Clock(param), value) = event.data else {
                continue;
            };
            self.clock_context
                .events_in
                .push(event.timestamp, UnifiedEvent::Parameter(param, value));
        }

        self.clock_context
            .process_with(context.stream_context, |ctx| self.clock.process(ctx));

        for (range, events) in self
            .clock_context
            .events_out
            .chunk_events(context.stream_context.block_size)
        {
            for event in events {
                match event.data {
                    UnifiedEvent::Parameter(mood::clock::ParamsOut::Tick, _) => {
                        self.current_sample = std::array::from_fn(|i| Stereo::from_usize(i))
                            .map(|ch| context.audio_in[ch][event.timestamp]);
                    }
                    _ => {}
                }
            }

            for ch in enum_iter::<Stereo>() {
                context.audio_out[ch][range.clone()].fill(self.current_sample[ch.to_usize()]);
            }
        }

        ProcessResult { tail: None }
    }
}

impl PluginDsp for Dsp {
    type Plugin = crate::Mood;

    fn create(context: PluginCreateContext<Self>, _: &<Self::Plugin as Plugin>::SharedData) -> Self {
        Self {
            clock_context: OwnedProcessContext::new(context.audio_config.max_frames_count as _, 512),
            clock: Clock::new(
                context.audio_config.sample_rate as _,
                context.params[Params::Clock(mood::clock::Params::Frequency)],
                context.params[Params::Clock(mood::clock::Params::Jitter)],
            ),
            current_sample: [0.0; 2],
        }
    }
}
