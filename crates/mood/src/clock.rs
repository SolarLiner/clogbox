use crate::rng::Lcg;
use clogbox_enum::{Empty, Enum};
use clogbox_module::context::{ProcessContext, UnifiedEvent};
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
use clogbox_oscillators::Phasor;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Enum)]
pub enum Params {
    Frequency,
    Jitter,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Enum)]
pub enum ParamsOut {
    Tick,
}

#[derive(Debug, Copy, Clone)]
pub struct Clock {
    phasor: Phasor<f32>,
    base_frequency: f32,
    jitter_rng: Lcg,
    jitter: f32,
}

impl Module for Clock {
    type Sample = f32;
    type AudioIn = Empty;
    type AudioOut = Empty;
    type ParamsIn = Params;
    type ParamsOut = ParamsOut;
    type NoteIn = Empty;
    type NoteOut = Empty;

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
        self.phasor.prepare(sample_rate, block_size);
        PrepareResult { latency: 0.0 }
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        for (range, events) in context
            .events_in
            .slice()
            .chunk_events(context.stream_context.block_size)
        {
            for event in events {
                match event.data {
                    UnifiedEvent::Parameter(Params::Frequency, value) => {
                        self.base_frequency = value;
                        self.set_next_frequency();
                    }
                    UnifiedEvent::Parameter(Params::Jitter, value) => {
                        self.jitter = value;
                    }
                    _ => {}
                }
            }
            for i in range {
                let rollovers = self.phasor.advance(1);
                if rollovers > 0 {
                    self.set_next_frequency();
                    context
                        .events_out
                        .push(i, UnifiedEvent::Parameter(ParamsOut::Tick, rollovers as _));
                }
            }
        }
        ProcessResult { tail: None }
    }
}

impl Clock {
    pub fn new(sample_rate: f32, base_frequency: f32, jitter: f32) -> Self {
        Self {
            phasor: Phasor::new(sample_rate, base_frequency),
            base_frequency,
            jitter_rng: Lcg::new(0x12345678),
            jitter,
        }
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.jitter_rng = Lcg::new(seed);
    }

    fn set_next_frequency(&mut self) {
        let jitter = self.get_jitter();
        self.phasor.set_frequency((self.base_frequency + jitter).max(1.0));
    }

    fn get_jitter(&mut self) -> f32 {
        let rand = self.jitter_rng.next_f32();
        let jitter = 2.0 * rand - 1.0;
        jitter * self.jitter * self.base_frequency
    }
}
