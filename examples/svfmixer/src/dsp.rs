use crate::params;
use clogbox_clap::processor::{PluginCreateContext, PluginDsp};
use clogbox_enum::enum_map::{EnumMapArray, EnumMapRef};
use clogbox_enum::Stereo;
use clogbox_filters::svf::{Svf, SvfOutput};
use clogbox_filters::{sinh, SimpleSaturator};
use clogbox_math::interpolation::Linear;
use clogbox_module::sample::{SampleModule, SampleModuleWrapper, SampleProcessResult};
use clogbox_module::{Module, PrepareResult, ProcessContext, ProcessResult, Samplerate, StreamContext};
use clogbox_params::smoothers::{LinearSmoother, Smoother};

pub struct DspPerSample {
    smoothers: EnumMapArray<params::Param, LinearSmoother<f32>>,
    dsp: EnumMapArray<Stereo, Svf<SimpleSaturator<f32>>>,
    buffer: EnumMapArray<Stereo, f32>,
}

impl SampleModule for DspPerSample {
    type Sample = f32;
    type AudioIn = Stereo;
    type AudioOut = Stereo;
    type Params = params::Param;

    fn prepare(&mut self, _sample_rate: Samplerate) -> PrepareResult {
        PrepareResult { latency: 2.0 }
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
        let outputs = self.process_sample(inputs);
        SampleProcessResult {
            output: outputs,
            tail: None,
        }
    }
}

type MW = SampleModuleWrapper<DspPerSample>;

impl DspPerSample {
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

pub struct Dsp(MW);

impl Module for Dsp {
    type Sample = <MW as Module>::Sample;
    type AudioIn = <MW as Module>::AudioIn;
    type AudioOut = <MW as Module>::AudioOut;
    type ParamsIn = <MW as Module>::ParamsIn;
    type ParamsOut = <MW as Module>::ParamsOut;
    type NoteIn = <MW as Module>::NoteIn;
    type NoteOut = <MW as Module>::NoteOut;

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
        self.0.prepare(sample_rate, block_size)
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        let context: ProcessContext<MW> = ProcessContext {
            audio_in: context.audio_in,
            audio_out: context.audio_out,
            params_in: context.params_in,
            params_out: context.params_out,
            note_in: context.note_in,
            note_out: context.note_out,
            stream_context: context.stream_context,
            __phantom: Default::default(),
        };
        self.0.process(context)
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
        let module = DspPerSample {
            smoothers,
            dsp,
            buffer: EnumMapArray::new(|_| 0.0),
        };
        Self(SampleModuleWrapper::new(
            module,
            EnumMapArray::new(|p| context.params[p]),
        ))
    }
}
