use crate::chorus::dsp;
use crate::chorus::dsp::Params;
use clogbox_enum::Empty;
use clogbox_module::context::{ProcessContext, UnifiedEvent};
use clogbox_module::eventbuffer::TimestampedCollection;
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
use clogbox_oscillators::Phasor;
use clogbox_params::smoothers::{ExpSmoother, Smoother};

pub(super) struct Lfo {
    pub(super) phasor: Phasor<f32>,
    pub(super) mod_amount: ExpSmoother<f32>,
    pub(super) out_smoother: ExpSmoother<f32>,
    lfo_inhibit: f32,
}

impl Lfo {
    pub const MIN_FREQUENCY: f32 = 25.3e3;
    pub const MAX_FREQUENCY: f32 = 200e3;
    pub const MID_FREQUENCY: f32 = Self::MIN_FREQUENCY + (Self::MAX_FREQUENCY - Self::MIN_FREQUENCY) / 2.0;

    pub(super) fn new(sample_rate: Samplerate) -> Self {
        Self {
            phasor: Phasor::new(sample_rate.value() as _, 1.0),
            mod_amount: ExpSmoother::new(sample_rate.value() as _, 5e-3, 0.0, 0.0),
            out_smoother: ExpSmoother::new(sample_rate.value() as _, 13.8e-3, 0.0, 0.0),
            lfo_inhibit: 0.0,
        }
    }
}

impl Module for Lfo {
    type Sample = f32;
    type AudioIn = Empty;
    type AudioOut = mood::clock::AudioIn;
    type ParamsIn = Params;
    type ParamsOut = Empty;
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
                    UnifiedEvent::Parameter(param, value) => match param {
                        Params::Rate => {
                            self.phasor.set_frequency(value);
                            self.lfo_inhibit = 1.0 / (1.0 + value - 0.3);
                        }
                        Params::Amount => {
                            self.mod_amount.set_target(value);
                        }
                        _ => {}
                    },
                    UnifiedEvent::Note(..) => {}
                }
            }

            for i in range {
                let mod_amount = self.mod_amount.next_value();
                let (t, _) = self.phasor.process_sample();
                let lfo = 2.0 * triangle(t) - 1.0;
                let lfo = lfo * self.lfo_inhibit;
                let lfo = 0.5 * lfo + 0.5;
                self.out_smoother.set_target(dsp::lerp(0.5..=lfo, mod_amount));
                context.audio_out[mood::clock::AudioIn::Frequency][i] = dsp::lerp(
                    Self::MIN_FREQUENCY..=Self::MAX_FREQUENCY,
                    self.out_smoother.next_value(),
                );
            }
        }
        ProcessResult { tail: None }
    }
}

fn triangle(x: f32) -> f32 {
    (x * 2.0 - 1.0).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_sweep() {
        let x: [f32; 512] = std::array::from_fn(|i| i as f32 / 512.0);
        let y = x.map(triangle);

        let out: [_; 512] = std::array::from_fn(|i| (x[i], y[i]));
        insta::assert_csv_snapshot!(&out as &[_]);
    }
}
