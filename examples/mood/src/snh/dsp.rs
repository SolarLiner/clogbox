use clogbox_clap::params::{frequency, linear, DynMapping, MappingExt, ParamId};
use clogbox_clap::{Plugin, PluginCreateContext, PluginDsp};
use clogbox_enum::{Empty, Enum, Stereo};
use clogbox_module::context::{EventBuffer, ProcessContext, UnifiedEvent};
use clogbox_module::eventbuffer::TimestampedCollectionMut;
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
use mood::bucket_brigade::BucketBrigade;
use std::borrow::Cow;
use std::fmt::Write;
use std::num::NonZeroU32;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Params(mood::clock::Params);

impl Enum for Params {
    type Count = <mood::clock::Params as Enum>::Count;

    fn from_usize(value: usize) -> Self {
        Self(mood::clock::Params::from_usize(value))
    }

    fn to_usize(self) -> usize {
        self.0.to_usize()
    }

    fn name(&self) -> Cow<str> {
        self.0.name()
    }
}

impl ParamId for Params {
    fn text_to_value(&self, text: &str) -> Option<f32> {
        text.parse().ok()
    }

    fn default_value(&self) -> f32 {
        use mood::clock::Params;
        match self.0 {
            Params::Frequency => 8000.0,
            Params::Jitter => 0.0,
        }
    }

    fn mapping(&self) -> DynMapping {
        use mood::clock::Params;
        match self.0 {
            Params::Frequency => frequency(2e3, 200e3).into_dyn(),
            Params::Jitter => linear(0.0, 1.0).into_dyn(),
        }
    }

    fn value_to_text(&self, f: &mut dyn Write, denormalized: f32) -> std::fmt::Result {
        use mood::clock::Params;
        match self.0 {
            Params::Frequency => write!(f, "{denormalized:3.2} Hz"),
            Params::Jitter => write!(f, "{:3.2} %", denormalized * 100.0),
        }
    }
}

type Chip = BucketBrigade<f32, Stereo, 1024>;

pub struct Dsp {
    bbd: Chip,
    bbd_events_in: EventBuffer<mood::clock::Params, Empty>,
    dummy_events_out: EventBuffer<Empty, Empty>,
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
        self.bbd_events_in = EventBuffer::new(block_size);
        self.bbd.prepare(sample_rate, block_size);
        PrepareResult { latency: 0.0 }
    }

    fn process(&mut self, mut context: ProcessContext<Self>) -> ProcessResult {
        self.process_events(&context);
        self.process_snh(&mut context);

        const TAIL: Option<NonZeroU32> = NonZeroU32::new(1);
        ProcessResult { tail: TAIL }
    }
}

impl Dsp {
    const HIGH_SHELF_PRE_GAIN: f32 = 5.7;
    const HIGH_SHELF_POST_GAIN: f32 = Self::HIGH_SHELF_PRE_GAIN.recip();
    fn process_events(&mut self, context: &ProcessContext<Dsp>) {
        self.bbd_events_in.clear();
        
        for event in context.events_in.slice() {
            let UnifiedEvent::Parameter(Params(param), value) = event.data else {
                continue;
            };
            self.bbd_events_in.push(event.timestamp, UnifiedEvent::Parameter(param, value));
        }
    }

    fn process_snh(&mut self, context: &mut ProcessContext<Dsp>) {
        let inner_context = ProcessContext {
            audio_in: context.audio_in,
            audio_out: context.audio_out,
            events_in: &self.bbd_events_in,
            events_out: &mut self.dummy_events_out,
            stream_context: context.stream_context,
            __phantom: Default::default(),
        };
        self.bbd.process(inner_context);
    }
}

impl PluginDsp for Dsp {
    type Plugin = super::MoodBBD;

    fn create(context: PluginCreateContext<Self>, _: &<Self::Plugin as Plugin>::SharedData) -> Self {
        let samplerate = Samplerate::new(context.audio_config.sample_rate);
        Self {
            bbd: BucketBrigade::new(
                context.audio_config.sample_rate as _,
                context.audio_config.max_frames_count as _,
                context.params[Params(mood::clock::Params::Frequency)],
                context.params[Params(mood::clock::Params::Jitter)],
            ),
            bbd_events_in: EventBuffer::new(512),
            dummy_events_out: EventBuffer::new(0),
        }
    }
}

fn lerp(range: std::ops::RangeInclusive<f32>, value: f32) -> f32 {
    let (start, end) = range.into_inner();
    let range = end - start;
    start + range * value
}
