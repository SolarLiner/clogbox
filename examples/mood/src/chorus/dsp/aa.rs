use crate::chorus::dsp;
use clogbox_filters::Multimode;

pub struct AntialiasFilter {
    filters: [Multimode<f32>; 4],
    hp4: f32,
}

impl AntialiasFilter {
    const DAMPING_RATIO: f32 = 0.5;
    const HP_FC: f32 = 48.2;
    const FC1: f32 = 6591.0;
    const FC2: f32 = 6934.0;

    pub(super) fn new(sample_rate: f32) -> Self {
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

    pub(super) fn next_sample(&mut self, input: f32) -> f32 {
        let lp1 = self.filters[0].next_sample(input);
        let hp1 = input - lp1;
        let lp2 = self.filters[1].next_sample(hp1);

        let in3 = lp2 + self.hp4 * Self::DAMPING_RATIO;
        let lp3 = self.filters[2].next_sample(in3);
        let lp4 = self.filters[3].next_sample(dsp::sat_bjt(lp3));
        self.hp4 = lp3 - lp4;
        lp4
    }
}
