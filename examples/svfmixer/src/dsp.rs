use crate::params;
use clogbox_clap::processor::{PluginCreateContext, PluginDsp, PluginError, ProcessStatus};
use clogbox_enum::enum_map::{EnumMapArray, EnumMapMut, EnumMapRef};
use clogbox_enum::{enum_iter, Stereo};
use clogbox_filters::svf::{Svf, SvfOutput};
use clogbox_filters::{sinh, SimpleSaturator};
use clogbox_math::interpolation::Linear;
use clogbox_params::smoothers::{LinearSmoother, Smoother};
use std::ops;

pub(crate) struct Dsp {
    smoothers: EnumMapArray<params::Param, LinearSmoother<f32>>,
    dsp: EnumMapArray<Stereo, Svf<SimpleSaturator<f32>>>,
    buffer: EnumMapArray<Stereo, f32>,
}

impl PluginDsp for Dsp {
    type Plugin = super::SvfMixer;
    type Params = params::Param;
    type Inputs = Stereo;
    type Outputs = Stereo;

    fn create(context: PluginCreateContext<Self>) -> Self {
        use params::Param::*;
        let samplerate = 2.0 * context.audio_config.sample_rate as f32;
        let smoothers =
            EnumMapArray::new(|p| LinearSmoother::new(Linear, samplerate, 10e-3, context.params[p], context.params[p]));
        let dsp = EnumMapArray::new(|_| {
            Svf::new(samplerate, context.params[Cutoff], context.params[Resonance]).with_saturator(sinh())
        });
        Self {
            smoothers,
            dsp,
            buffer: EnumMapArray::new(|_| 0.0),
        }
    }

    fn set_param(&mut self, id: Self::Params, value: f32) {
        self.smoothers[id].set_target(value);
    }

    fn process<In: ops::Deref<Target = [f32]>, Out: ops::DerefMut<Target = [f32]>>(
        &mut self,
        frame_count: usize,
        inputs: EnumMapRef<Self::Inputs, In>,
        mut outputs: EnumMapMut<Self::Outputs, Out>,
    ) -> Result<ProcessStatus, PluginError> {
        use params::Param::*;
        for i in 0..frame_count {
            let params = EnumMapArray::new(|p| self.smoothers[p].next_value());
            for dsp in self.dsp.values_mut() {
                dsp.set_cutoff_no_update(params[Cutoff]);
                dsp.set_resonance(params[Resonance]);
            }
            for ch in enum_iter::<Stereo>() {
                let x = inputs[ch][i].clamp(-0.95, 0.95);
                outputs[ch][i] = self.next_sample(ch, x);
            }
        }
        Ok(ProcessStatus::ContinueIfNotQuiet)
    }
}

impl Dsp {
    fn next_sample(&mut self, channel: Stereo, sample: f32) -> f32 {
        // Crude 2x oversampling to decramp
        let a = self.next_sample_inner(channel, self.buffer[channel]);
        let b = self.next_sample_inner(channel, (sample + self.buffer[channel]) / 2.0);
        self.buffer[channel] = sample;
        (a + b) / 2.0
    }

    fn next_sample_inner(&mut self, channel: Stereo, sample: f32) -> f32 {
        use params::Param::*;
        let params = if channel == Stereo::Left {
            EnumMapArray::new(|i| self.smoothers[i].next_value())
        } else {
            EnumMapArray::new(|i| self.smoothers[i].current_value())
        };
        let out = self.dsp[channel].next_sample(sample);
        let y = params[Lowpass] * out[SvfOutput::Lowpass]
            + params[Bandpass] * out[SvfOutput::Bandpass]
            + params[Highpass] * out[SvfOutput::Highpass];
        y
    }
}
