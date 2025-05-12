use crate::params;
use clogbox_clap::processor::{PluginCreateContext, PluginDsp};
use clogbox_enum::enum_map::{EnumMapArray, EnumMapRef};
use clogbox_enum::{Enum, Stereo};
use clogbox_filters::svf::{FilterType, Svf, SvfImpl, SvfMixer, SvfSampleOutput};
use clogbox_math::interpolation::Linear;
use clogbox_math::root_eq::nr::NewtonRaphson;
use clogbox_module::sample::{SampleModule, SampleModuleWrapper, SampleProcessResult};
use clogbox_module::{module_wrapper, Module, PrepareResult, Samplerate};
use clogbox_params::smoothers::{LinearSmoother, Smoother};

use crate::params::Param;
use clogbox_module::context::StreamContext;
use nalgebra as na;

struct OtaTanh;

impl SvfImpl<f32> for OtaTanh {
    #[inline]
    fn next_sample(svf: &mut Svf<f32, Self>, input: f32) -> SvfSampleOutput<f32> {
        const NR: NewtonRaphson<f32> = NewtonRaphson {
            over_relaxation: 1.0,
            max_iterations: 500,
            tolerance: 1e-4,
        };

        let mut x = na::OVector::from(svf.last_out);
        NR.solve_multi(
            &crate::gen::SvfEquation {
                S: svf.s.into(),
                x: input,
                g: svf.g,
                k_drive: svf.drive,
                q: svf.q,
            },
            x.as_view_mut(),
        );
        SvfSampleOutput {
            y: x.into(),
            s: crate::gen::state(svf.s.into(), svf.g, svf.drive, x[1], x[2], x[0]).into(),
        }
    }
}

pub struct DspPerSample {
    smoothers: EnumMapArray<params::Param, LinearSmoother<f32>>,
    dsp: EnumMapArray<Stereo, Svf<f32, OtaTanh>>,
    mixer: EnumMapArray<Stereo, SvfMixer<f32>>,
    buffer: EnumMapArray<Stereo, f32>,
}

impl SampleModule for DspPerSample {
    type Sample = f32;
    type AudioIn = Stereo;
    type AudioOut = Stereo;
    type Params = params::Param;

    fn prepare(&mut self, _sample_rate: Samplerate) -> PrepareResult {
        PrepareResult { latency: 0.0 }
    }

    fn process(
        &mut self,
        _stream_context: &StreamContext,
        inputs: EnumMapArray<Self::AudioIn, Self::Sample>,
        params: EnumMapRef<Self::Params, f32>,
    ) -> SampleProcessResult<Self::AudioOut, Self::Sample> {
        for (param, smoother) in self.smoothers.iter_mut() {
            smoother.set_target(params[param]);
        }
        for mixer in self.mixer.values_mut() {
            let value = params[Param::FilterType];
            let ftype = FilterType::from_usize(value.round() as _);
            mixer.set_filter_type(ftype);
        }
        let outputs = self.process_sample(inputs);
        SampleProcessResult {
            output: outputs,
            tail: None,
        }
    }
}

impl DspPerSample {
    fn process_sample(&mut self, input: EnumMapArray<Stereo, f32>) -> EnumMapArray<Stereo, f32> {
        use params::Param::*;
        let params = EnumMapArray::new(|p| self.smoothers[p].next_value());
        for dsp in self.dsp.values_mut() {
            dsp.set_cutoff_no_update(params[Cutoff]);
            dsp.set_resonance(params[Resonance]);
            dsp.set_drive(params[Drive]);
        }
        for mixer in self.mixer.values_mut() {
            mixer.set_amp(params[Gain]);
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
        let filter_out = self.dsp[channel].next_sample(sample);
        self.mixer[channel].mix(sample, filter_out)
    }
}

module_wrapper!(Dsp: SampleModuleWrapper<DspPerSample>);

impl PluginDsp for Dsp {
    type Plugin = super::SvfMixer;

    fn create(context: PluginCreateContext<Self>, _: &()) -> Self {
        use params::Param::*;
        let samplerate = 2.0 * context.audio_config.sample_rate as f32;
        let smoothers =
            EnumMapArray::new(|p| LinearSmoother::new(Linear, samplerate, 10e-3, context.params[p], context.params[p]));
        let dsp = EnumMapArray::new(|_| Svf::new(samplerate, context.params[Cutoff], context.params[Resonance]));
        let mixer = EnumMapArray::new(|_| {
            SvfMixer::new(
                samplerate,
                clogbox_filters::svf::FilterType::from_usize(context.params[FilterType].round() as usize),
                context.params[Gain],
            )
        });
        let module = DspPerSample {
            smoothers,
            dsp,
            mixer,
            buffer: EnumMapArray::new(|_| 0.0),
        };
        Self(SampleModuleWrapper::new(
            module,
            EnumMapArray::new(|p| context.params[p]),
        ))
    }
}
