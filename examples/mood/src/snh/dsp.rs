use clogbox_clap::params::{frequency, linear, DynMapping, MappingExt, ParamId};
use clogbox_clap::{Plugin, PluginCreateContext, PluginDsp};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{enum_iter, Empty, Enum, Stereo};
use clogbox_filters::Multimode;
use clogbox_module::context::{AudioStorage, OwnedProcessContext, ProcessContext, UnifiedEvent};
use clogbox_module::eventbuffer::TimestampedCollectionMut;
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
use clogbox_params::smoothers::{ExpSmoother, Smoother};
use mood::clock::Clock;
use std::fmt::Write;
// use clogbox_clap_egui::egui::lerp;

struct HighShelf {
    filter: Multimode<f32>,
    gain: ExpSmoother<f32>,
}

impl HighShelf {
    const CUTOFF: f32 = 410.0;
    fn new(sample_rate: Samplerate, gain: f32) -> Self {
        Self {
            filter: Multimode::new(sample_rate.value() as _, Self::CUTOFF),
            gain: ExpSmoother::new(sample_rate.value() as _, 1e-3, gain, gain),
        }
    }

    fn prepare(&mut self, samplerate: Samplerate) {
        self.filter.set_samplerate(samplerate.value() as _);
        self.gain.set_samplerate(samplerate.value() as _);
    }

    fn set_gain(&mut self, gain: f32) {
        self.gain.set_target(gain);
    }

    fn process_sample(&mut self, input: f32) -> f32 {
        let lp = self.filter.next_sample(input);
        let hp = input - lp;
        let gain = self.gain.next_value();
        input + hp * (gain - 1.0)
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum Params {
    Clock(mood::clock::Params),
    Emphasis,
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
            Self::Emphasis => 1.0,
        }
    }

    fn mapping(&self) -> DynMapping {
        use mood::clock::Params::*;
        match self {
            Self::Clock(Frequency) => frequency(20.0, 40000.0).into_dyn(),
            Self::Clock(Jitter) => linear(0.0, 1.0).into_dyn(),
            Self::Emphasis => linear(0.0, 1.0).into_dyn(),
        }
    }

    fn value_to_text(&self, f: &mut dyn Write, denormalized: f32) -> std::fmt::Result {
        use mood::clock::Params::*;
        match self {
            Self::Clock(Frequency) => write!(f, "{denormalized:3.2} Hz"),
            Self::Clock(Jitter) | Self::Emphasis => write!(f, "{:3.2} %", denormalized * 100.0),
        }
    }
}

pub struct Dsp {
    clock_context: OwnedProcessContext<Clock>,
    pre_emphasis: EnumMapArray<Stereo, HighShelf>,
    clock: Clock,
    post_emphasis: EnumMapArray<Stereo, HighShelf>,
    scratch_buffer1: AudioStorage<Stereo, f32>,
    scratch_buffer2: AudioStorage<Stereo, f32>,
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
        for ch in enum_iter::<Stereo>() {
            self.pre_emphasis[ch].prepare(sample_rate);
            self.post_emphasis[ch].prepare(sample_rate);
        }
        self.scratch_buffer1 = AudioStorage::zeroed(block_size);
        self.scratch_buffer2 = AudioStorage::zeroed(block_size);
        PrepareResult { latency: 0.0 }
    }

    fn process(&mut self, mut context: ProcessContext<Self>) -> ProcessResult {
        self.clock_context.events_in.clear();
        self.clock_context.events_out.clear();

        self.process_events(&context);

        self.clock_context
            .process_with(context.stream_context, |ctx| self.clock.process(ctx));

        self.process_preemphasis(&mut context);
        self.process_snh(&mut context);
        self.process_postemphasis(&mut context);

        ProcessResult { tail: None }
    }
}

impl Dsp {
    const HIGH_SHELF_PRE_GAIN: f32 = 5.7;
    const HIGH_SHELF_POST_GAIN: f32 = Self::HIGH_SHELF_PRE_GAIN.recip();
    fn process_events(&mut self, context: &ProcessContext<Dsp>) {
        for event in context.events_in.slice() {
            let UnifiedEvent::Parameter(params, value) = event.data else {
                continue;
            };
            match params {
                Params::Clock(param) => {
                    self.clock_context
                        .events_in
                        .push(event.timestamp, UnifiedEvent::Parameter(param, value));
                }
                Params::Emphasis => {
                    self.pre_emphasis
                        .values_mut()
                        .for_each(|filter| filter.set_gain(lerp(1.0..=Self::HIGH_SHELF_PRE_GAIN, value)));
                    self.post_emphasis
                        .values_mut()
                        .for_each(|filter| filter.set_gain(lerp(1.0..=Self::HIGH_SHELF_POST_GAIN, value)));
                }
            }
        }
    }

    fn process_preemphasis(&mut self, context: &mut ProcessContext<Dsp>) {
        for ch in enum_iter::<Stereo>() {
            for i in 0..context.stream_context.block_size {
                let out = self.pre_emphasis[ch].process_sample(context.audio_in[ch][i]);
                self.scratch_buffer1[ch][i] = out;
            }
        }
    }

    fn process_postemphasis(&mut self, context: &mut ProcessContext<Dsp>) {
        for ch in enum_iter::<Stereo>() {
            for i in 0..context.stream_context.block_size {
                let out = self.post_emphasis[ch].process_sample(self.scratch_buffer2[ch][i]);
                context.audio_out[ch][i] = out;
            }
        }
    }

    fn process_snh(&mut self, context: &mut ProcessContext<Dsp>) {
        for (range, events) in self
            .clock_context
            .events_out
            .chunk_events(context.stream_context.block_size)
        {
            for event in events {
                match event.data {
                    UnifiedEvent::Parameter(mood::clock::ParamsOut::Tick, _) => {
                        self.current_sample = std::array::from_fn(|i| Stereo::from_usize(i)).map(|ch| {
                            let x = self.scratch_buffer1[ch][event.timestamp];
                            (x / 2.0).tanh() * 2.0
                        });
                    }
                    _ => {}
                }
            }

            for ch in enum_iter::<Stereo>() {
                self.scratch_buffer2[ch][range.clone()].fill(self.current_sample[ch.to_usize()]);
            }
        }
    }
}

impl PluginDsp for Dsp {
    type Plugin = super::MoodSnh;

    fn create(context: PluginCreateContext<Self>, _: &<Self::Plugin as Plugin>::SharedData) -> Self {
        let samplerate = Samplerate::new(context.audio_config.sample_rate);
        let emphasis_amt = context.params[Params::Emphasis];
        let create_emphasis = |gain| {
            move |_| {
                let mut filter = HighShelf::new(samplerate, 1.0);
                filter.set_gain(lerp(1.0..=gain, emphasis_amt));
                filter
            }
        };
        Self {
            clock_context: OwnedProcessContext::new(context.audio_config.max_frames_count as _, 512),
            clock: Clock::new(
                context.audio_config.sample_rate as _,
                context.params[Params::Clock(mood::clock::Params::Frequency)],
                context.params[Params::Clock(mood::clock::Params::Jitter)],
            ),
            pre_emphasis: EnumMapArray::new(create_emphasis(Self::HIGH_SHELF_PRE_GAIN)),
            post_emphasis: EnumMapArray::new(create_emphasis(Self::HIGH_SHELF_POST_GAIN)),
            scratch_buffer1: AudioStorage::zeroed(context.audio_config.max_frames_count as _),
            scratch_buffer2: AudioStorage::zeroed(context.audio_config.max_frames_count as _),
            current_sample: [0.0; 2],
        }
    }
}

fn lerp(range: std::ops::RangeInclusive<f32>, value: f32) -> f32 {
    let (start, end) = range.into_inner();
    let range = end - start;
    start + range * value
}
