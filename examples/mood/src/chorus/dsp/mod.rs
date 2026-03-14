use aa::AntialiasFilter;
use clogbox_clap::params::{linear, DynMapping, MappingExt, ParamId};
use clogbox_clap::{Plugin, PluginCreateContext, PluginDsp};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{enum_iter, Empty, Enum, Stereo};
use clogbox_filters::saturators::SimpleSaturator;
use clogbox_module::context::{AudioStorage, EventBuffer, OwnedProcessContext, ProcessContext, UnifiedEvent};
use clogbox_module::eventbuffer::TimestampedCollectionMut;
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
use clogbox_params::smoothers::Smoother;
use highshelf::HighShelf;
use lfo::Lfo;
use mood::bucket_brigade::BucketBrigade;
use std::fmt::Write;
use std::marker::PhantomData;
use std::num::NonZeroU32;

mod aa;
mod highshelf;
mod lfo;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum Params {
    Rate,
    Amount,
}

impl ParamId for Params {
    fn text_to_value(&self, text: &str) -> Option<f32> {
        text.parse().ok()
    }

    fn default_value(&self) -> f32 {
        match self {
            Self::Rate => 1.0,
            Self::Amount => 0.5,
        }
    }

    fn mapping(&self) -> DynMapping {
        match self {
            Self::Rate => linear(0.3, 3.57).into_dyn(),
            Self::Amount => linear(0.0, 1.0).into_dyn(),
        }
    }

    fn value_to_text(&self, f: &mut dyn Write, denormalized: f32) -> std::fmt::Result {
        match self {
            Self::Rate => write!(f, "{denormalized:1.1} Hz"),
            Self::Amount => write!(f, "{:>3.2}%", 100.0 * denormalized),
        }
    }
}

type Chip = BucketBrigade<f32, Stereo, 1024>;

pub struct Dsp {
    lfo_context: OwnedProcessContext<Lfo>,
    lfo: Lfo,
    chip_context: OwnedProcessContext<Chip>,
    chip: Chip,
    pre_aa: EnumMapArray<Stereo, AntialiasFilter>,
    post_aa: EnumMapArray<Stereo, AntialiasFilter>,
    pre_emphasis: EnumMapArray<Stereo, HighShelf>,
    post_emphasis: EnumMapArray<Stereo, HighShelf>,
    scratch_buffer1: AudioStorage<Stereo, f32>,
    scratch_buffer2: AudioStorage<Stereo, f32>,
    lfo_buffer: AudioStorage<mood::clock::AudioIn, f32>,
    dummy_buffer: AudioStorage<Empty, f32>,
    dummy_events: EventBuffer<Empty, Empty>,
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
        self.lfo_context.resize_audio_buffers(block_size);
        self.lfo.prepare(sample_rate, block_size);
        self.chip_context.resize_audio_buffers(block_size);
        self.chip.prepare(sample_rate, block_size);
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
        self.process_lfo(&context);
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
                Params::Rate => {
                    self.lfo.phasor.set_frequency(value);
                }
                Params::Amount => {
                    self.lfo.mod_amount.set_target(value);
                }
            }
        }
    }

    fn process_lfo(&mut self, context: &ProcessContext<Dsp>) {
        self.lfo.process(ProcessContext {
            stream_context: context.stream_context,
            audio_in: &self.dummy_buffer,
            audio_out: &mut self.lfo_buffer,
            events_in: context.events_in,
            events_out: &mut self.dummy_events,
            __phantom: PhantomData,
        });
    }

    fn process_preemphasis(&mut self, context: &mut ProcessContext<Dsp>) {
        const INPUT_GAIN: f32 = 0.5;
        for ch in enum_iter::<Stereo>() {
            for i in 0..context.stream_context.block_size {
                let inp = sat_bjt(INPUT_GAIN * context.audio_in[ch][i]);
                let out = self.pre_aa[ch].next_sample(inp);
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
        for ch in enum_iter::<Stereo>() {
            self.chip_context.audio_in[mood::bucket_brigade::AudioIn::Audio(ch)].copy_from_slice(&context.audio_in[ch]);
        }
        self.chip_context.audio_in[mood::bucket_brigade::AudioIn::Frequency]
            .copy_from_slice(&self.lfo_buffer[mood::clock::AudioIn::Frequency]);
        self.chip_context
            .process_with(context.stream_context, |ctx| self.chip.process(ctx));
        self.scratch_buffer2.copy_from_input(&self.chip_context.audio_out);
    }
}

impl PluginDsp for Dsp {
    type Plugin = super::MoodChorus;

    fn create(context: PluginCreateContext<Self>, _: &<Self::Plugin as Plugin>::SharedData) -> Self {
        let samplerate = Samplerate::new(context.audio_config.sample_rate);
        let create_emphasis = |gain| move |_| HighShelf::new(samplerate, gain);
        Self {
            lfo_context: OwnedProcessContext::new(context.audio_config.max_frames_count as _, 512),
            lfo: Lfo::new(samplerate),
            chip_context: OwnedProcessContext::new(context.audio_config.max_frames_count as _, 512),
            chip: BucketBrigade::new(
                context.audio_config.sample_rate as _,
                context.audio_config.max_frames_count as _,
                Lfo::MID_FREQUENCY,
                0.01,
            )
            .with_saturator(SimpleSaturator::new(sat_bbd)),
            pre_aa: EnumMapArray::new(|_| AntialiasFilter::new(samplerate.value() as _)),
            post_aa: EnumMapArray::new(|_| AntialiasFilter::new(samplerate.value() as _)),
            pre_emphasis: EnumMapArray::new(create_emphasis(Self::HIGH_SHELF_PRE_GAIN)),
            post_emphasis: EnumMapArray::new(create_emphasis(Self::HIGH_SHELF_POST_GAIN)),
            scratch_buffer1: AudioStorage::zeroed(context.audio_config.max_frames_count as _),
            scratch_buffer2: AudioStorage::zeroed(context.audio_config.max_frames_count as _),
            lfo_buffer: AudioStorage::zeroed(context.audio_config.max_frames_count as _),
            dummy_buffer: AudioStorage::zeroed(0),
            dummy_events: EventBuffer::new(0),
        }
    }
}

fn lerp(range: std::ops::RangeInclusive<f32>, value: f32) -> f32 {
    let (start, end) = range.into_inner();
    let range = end - start;
    start + range * value
}

fn sat_bbd(x: f32) -> f32 {
    const BIAS: f32 = -4.5;
    const SCALE: f32 = 15.0;
    let inp = (x - BIAS) / SCALE;
    let out = inp.tanh() + (BIAS / SCALE).tanh();
    out * SCALE
}

fn sat_bjt(x: f32) -> f32 {
    const BIAS: f32 = 0.707;
    const SCALE: f32 = 4.5;
    let inp = (x - BIAS) / SCALE;
    let out = inp.tanh() + (BIAS / SCALE).tanh();
    out * SCALE
}
