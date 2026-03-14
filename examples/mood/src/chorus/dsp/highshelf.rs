use clogbox_filters::Multimode;
use clogbox_module::Samplerate;
use clogbox_params::smoothers::{ExpSmoother, Smoother};

pub(super) struct HighShelf {
    filter: Multimode<f32>,
    gain: ExpSmoother<f32>,
}

impl HighShelf {
    const CUTOFF: f32 = 410.0;
    pub(super) fn new(sample_rate: Samplerate, gain: f32) -> Self {
        Self {
            filter: Multimode::new(sample_rate.value() as _, Self::CUTOFF),
            gain: ExpSmoother::new(sample_rate.value() as _, 1e-3, gain, gain),
        }
    }

    pub(super) fn prepare(&mut self, samplerate: Samplerate) {
        self.filter.set_samplerate(samplerate.value() as _);
        self.gain.set_samplerate(samplerate.value() as _);
    }

    pub(super) fn set_gain(&mut self, gain: f32) {
        self.gain.set_target(gain);
    }

    pub(super) fn process_sample(&mut self, input: f32) -> f32 {
        let lp = self.filter.next_sample(input);
        let hp = input - lp;
        let gain = self.gain.next_value();
        input + hp * (gain - 1.0)
    }
}
