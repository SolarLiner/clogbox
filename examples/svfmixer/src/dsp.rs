use crate::params;
use clogbox_clap::processor::{PluginCreateContext, PluginDsp};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{enum_iter, Empty, Stereo};
use clogbox_filters::svf::{Svf, SvfOutput};
use clogbox_filters::{sinh, SimpleSaturator};
use clogbox_math::interpolation::Linear;
use clogbox_module::eventbuffer::Timestamped;
use clogbox_module::{Module, PrepareResult, ProcessContext, ProcessResult, Samplerate};
use clogbox_params::smoothers::{LinearSmoother, Smoother};
use std::num::NonZeroU32;

pub(crate) struct Dsp {
    smoothers: EnumMapArray<params::Param, LinearSmoother<f32>>,
    dsp: EnumMapArray<Stereo, Svf<SimpleSaturator<f32>>>,
    buffer: EnumMapArray<Stereo, f32>,
}

impl Module for Dsp {
    type Sample = f32;
    type AudioIn = Stereo;
    type AudioOut = Stereo;
    type ParamsIn = params::Param;
    type ParamsOut = Empty;
    type NoteIn = Empty;
    type NoteOut = Empty;

    fn prepare(&mut self, _sample_rate: Samplerate, _block_size: usize) -> PrepareResult {
        PrepareResult { latency: 2.0 }
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        let mut start = 0;
        let end = context.stream_context.block_size;
        while start < end {
            let end = enum_iter::<params::Param>()
                .filter_map(|e| context.params_in[e].after(start + 1).first().map(|t| t.timestamp))
                .min()
                .unwrap_or(context.stream_context.block_size);
            for param in enum_iter::<params::Param>() {
                let Some(&Timestamped { data, .. }) = context.params_in[param].at(start) else {
                    continue;
                };
                self.set_param(param, data);
            }
            for i in start..end {
                let output = self.process_sample(EnumMapArray::new(|ch| context.audio_in[ch][i]));
                context.audio_out[Stereo::Left][i] = output[Stereo::Left];
                context.audio_out[Stereo::Right][i] = output[Stereo::Right];
            }
            start = end;
        }
        ProcessResult {
            tail: NonZeroU32::new(2),
        }
    }
}

impl PluginDsp for Dsp {
    type Plugin = super::SvfMixer;

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
}

impl Dsp {
    fn set_param(&mut self, id: params::Param, value: f32) {
        self.smoothers[id].set_target(value);
    }

    fn process_sample(&mut self, input: EnumMapArray<Stereo, f32>) -> EnumMapArray<Stereo, f32> {
        use params::Param::*;
        let params = EnumMapArray::new(|p| self.smoothers[p].next_value());
        for dsp in self.dsp.values_mut() {
            dsp.set_cutoff_no_update(params[Cutoff]);
            dsp.set_resonance(params[Resonance]);
        }
        EnumMapArray::new(|ch| self.next_sample(ch, input[ch]))
    }

    fn next_sample(&mut self, channel: Stereo, sample: f32) -> f32 {
        // Crude 2x oversampling to decramp
        let a = self.next_sample_inner(channel, self.buffer[channel]);
        let b = self.next_sample_inner(channel, (sample + self.buffer[channel]) / 2.0);
        self.buffer[channel] = sample;
        (a + b) / 2.0
    }

    fn next_sample_inner(&mut self, channel: Stereo, sample: f32) -> f32 {
        use params::Param::*;
        let params = EnumMapArray::new(|i| self.smoothers[i].current_value());
        let out = self.dsp[channel].next_sample(sample.tanh());
        let y = params[Lowpass] * out[SvfOutput::Lowpass]
            + params[Bandpass] * out[SvfOutput::Bandpass]
            + params[Highpass] * out[SvfOutput::Highpass];
        y
    }
}
