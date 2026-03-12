use clogbox_clap::params::{frequency, linear, DynMapping, MappingExt, ParamId};
use clogbox_clap::{Plugin, PluginCreateContext, PluginDsp};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{enum_iter, Empty, Enum, Stereo};
use clogbox_filters::saturators::{tanh, Driven, SimpleSaturator};
use clogbox_filters::Multimode;
use clogbox_module::context::{AudioStorage, OwnedProcessContext, ProcessContext, UnifiedEvent};
use clogbox_module::eventbuffer::TimestampedCollectionMut;
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
use clogbox_params::smoothers::{ExpSmoother, Smoother};
use mood::bucket_brigade::BucketBrigade;
use std::fmt::Write;
use std::num::NonZeroU32;

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

struct AntialiasFilter {
    filters: [Multimode<f32>; 4],
    hp4: f32,
}

impl AntialiasFilter {
    const DAMPING_RATIO: f32 = 0.5;
    const HP_FC: f32 = 48.2;
    const FC1: f32 = 6591.0;
    const FC2: f32 = 6934.0;

    fn new(sample_rate: f32) -> Self {
        Self {
            filters: [
                Multimode::new(sample_rate, Self::HP_FC),
                Multimode::new(sample_rate, Self::FC1),
                Multimode::new(sample_rate, Self::FC2),
                Multimode::new(sample_rate, Self::FC2),
            ],
            hp4: 0.0,
        }
    }

    fn next_sample(&mut self, input: f32) -> f32 {
        let lp1 = self.filters[0].next_sample(input);
        let hp1 = input - lp1;
        let lp2 = self.filters[1].next_sample(hp1);

        let in3 = lp2 + self.hp4 * Self::DAMPING_RATIO;
        let lp3 = self.filters[2].next_sample(in3);
        let lp4 = self.filters[3].next_sample(sat_bjt(lp3));
        self.hp4 = lp3 - lp4;
        lp4
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
            Self::Clock(Frequency) => frequency(2e3, 200e3).into_dyn(),
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

type Chip = BucketBrigade<f32, Stereo, 1024>;

pub struct Dsp {
    chip_context: OwnedProcessContext<Chip>,
    bbd: Chip,
    pre_aa: EnumMapArray<Stereo, AntialiasFilter>,
    post_aa: EnumMapArray<Stereo, AntialiasFilter>,
    pre_emphasis: EnumMapArray<Stereo, HighShelf>,
    post_emphasis: EnumMapArray<Stereo, HighShelf>,
    scratch_buffer1: AudioStorage<Stereo, f32>,
    scratch_buffer2: AudioStorage<Stereo, f32>,
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
        self.chip_context.resize_audio_buffers(block_size);
        self.bbd.prepare(sample_rate, block_size);
        for ch in enum_iter::<Stereo>() {
            self.pre_emphasis[ch].prepare(sample_rate);
            self.post_emphasis[ch].prepare(sample_rate);
        }
        self.scratch_buffer1 = AudioStorage::zeroed(block_size);
        self.scratch_buffer2 = AudioStorage::zeroed(block_size);
        PrepareResult { latency: 0.0 }
    }

    fn process(&mut self, mut context: ProcessContext<Self>) -> ProcessResult {
        self.process_events(&context);
        self.process_preemphasis(&mut context);
        self.process_snh(&mut context);
        self.process_postemphasis(&mut context);

        const TAIL: Option<NonZeroU32> = NonZeroU32::new(1);
        ProcessResult { tail: TAIL }
    }
}

impl Dsp {
    const HIGH_SHELF_PRE_GAIN: f32 = 5.7;
    const HIGH_SHELF_POST_GAIN: f32 = Self::HIGH_SHELF_PRE_GAIN.recip();
    fn process_events(&mut self, context: &ProcessContext<Dsp>) {
        self.chip_context.events_in.clear();
        for event in context.events_in.slice() {
            let UnifiedEvent::Parameter(params, value) = event.data else {
                continue;
            };
            match params {
                Params::Clock(param) => {
                    self.chip_context
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
                let out = self.pre_aa[ch].next_sample(sat_bjt(context.audio_in[ch][i]));
                let out = self.pre_emphasis[ch].process_sample(out);
                self.scratch_buffer1[ch][i] = out;
            }
        }
    }

    fn process_postemphasis(&mut self, context: &mut ProcessContext<Dsp>) {
        for ch in enum_iter::<Stereo>() {
            for i in 0..context.stream_context.block_size {
                let out = self.post_emphasis[ch].process_sample(self.scratch_buffer2[ch][i]);
                let out = self.post_aa[ch].next_sample(out);
                context.audio_out[ch][i] = out;
            }
        }
    }

    fn process_snh(&mut self, context: &mut ProcessContext<Dsp>) {
        self.chip_context.audio_in.copy_from_input(&self.scratch_buffer1);
        self.chip_context
            .process_with(context.stream_context, |ctx| self.bbd.process(ctx));
        self.scratch_buffer2.copy_from_input(&self.chip_context.audio_out);
    }
}

impl PluginDsp for Dsp {
    type Plugin = super::MoodChorus;

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
            chip_context: OwnedProcessContext::new(context.audio_config.max_frames_count as _, 512),
            bbd: BucketBrigade::new(
                context.audio_config.sample_rate as _,
                context.audio_config.max_frames_count as _,
                context.params[Params::Clock(mood::clock::Params::Frequency)],
                context.params[Params::Clock(mood::clock::Params::Jitter)],
            )
            .with_saturator(tanh()),
            pre_aa: EnumMapArray::new(|_| AntialiasFilter::new(samplerate.value() as _)),
            post_aa: EnumMapArray::new(|_| AntialiasFilter::new(samplerate.value() as _)),
            pre_emphasis: EnumMapArray::new(create_emphasis(Self::HIGH_SHELF_PRE_GAIN)),
            post_emphasis: EnumMapArray::new(create_emphasis(Self::HIGH_SHELF_POST_GAIN)),
            scratch_buffer1: AudioStorage::zeroed(context.audio_config.max_frames_count as _),
            scratch_buffer2: AudioStorage::zeroed(context.audio_config.max_frames_count as _),
        }
    }
}

fn lerp(range: std::ops::RangeInclusive<f32>, value: f32) -> f32 {
    let (start, end) = range.into_inner();
    let range = end - start;
    start + range * value
}

fn sat_bjt(x: f32) -> f32 {
    const BIAS: f32 = 0.707;
    const SCALE: f32 = 4.5;
    ((x - BIAS) / SCALE).tanh() * SCALE + BIAS
}
