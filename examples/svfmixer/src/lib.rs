use clogbox_clap::{
    clap_module, features, ClapModule, HostSharedHandle, PluginDescriptor, PluginError,
};
use clogbox_core::module::utilitarian::{SummingMatrix, SummingMatrixParams};
use clogbox_core::module::{
    BufferStorage, Module, ModuleConstructor, ModuleContext, ProcessStatus, StreamData,
};
use clogbox_core::param::events::ParamEvents;
use clogbox_core::r#enum::enum_map::EnumMapRef;
use clogbox_core::r#enum::{enum_iter, Mono, Stereo};
use clogbox_derive::{Enum, Params};
use clogbox_filters::svf::{Svf, SvfInput, SvfOutput, SvfParams};
use clogbox_filters::{Memoryless, Saturator, SimpleSaturator};
use clogbox_graph::schedule::Schedule;
use clogbox_graph::ScheduleBuilder;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum, Params)]
pub enum PluginParams {
    SvfParam(SvfParams<<SimpleSaturator<f32> as Saturator>::Params>),
    MixerParam(SummingMatrixParams<SvfOutput, Mono>),
}

pub struct Plugin {
    schedule: Schedule<f32>,
}

impl Module for Plugin {
    type Sample = f32;
    type Inputs = Stereo;
    type Outputs = Stereo;
    type Params = PluginParams;

    fn supports_stream(&self, data: StreamData) -> bool {
        self.schedule.supports_stream(data)
    }

    fn reallocate(&mut self, stream_data: StreamData) {
        self.schedule.reallocate(stream_data)
    }

    fn process<
        S: BufferStorage<Sample = Self::Sample, Input = Self::Inputs, Output = Self::Outputs>,
    >(
        &mut self,
        context: &mut ModuleContext<S>,
        params: EnumMapRef<Self::Params, &dyn ParamEvents>,
    ) -> ProcessStatus {
        self.schedule.process(context, params)
    }
}

pub struct PluginConstructor;

impl ModuleConstructor for PluginConstructor {
    type Module = Plugin;

    fn allocate(&self, stream_data: StreamData) -> Self::Module {
        let mut builder = ScheduleBuilder::new();
        let in_left = builder.add_io_node(true).unwrap();
        let in_right = builder.add_io_node(true).unwrap();
        let out_left = builder.add_io_node(false).unwrap();
        let out_right = builder.add_io_node(false).unwrap();
        let svf_left = builder
            .add_node(Svf::new(stream_data.sample_rate as _, 3000.0, 0.5))
            .unwrap();
        let mixer_left = builder
            .add_node(SummingMatrix::<f32, SvfOutput, Mono>::new())
            .unwrap();
        builder
            .connect_input(in_left, svf_left, SvfInput::AudioInput)
            .unwrap();
        builder
            .connect(svf_left, mixer_left, SvfOutput::Lowpass, SvfOutput::Lowpass)
            .unwrap();
        builder
            .connect(
                svf_left,
                mixer_left,
                SvfOutput::Bandpass,
                SvfOutput::Bandpass,
            )
            .unwrap();
        builder
            .connect(
                svf_left,
                mixer_left,
                SvfOutput::Highpass,
                SvfOutput::Highpass,
            )
            .unwrap();
        builder
            .connect_output(mixer_left, out_left, Mono::Mono)
            .unwrap();

        let svf_right = builder
            .add_node(Svf::new(stream_data.sample_rate as _, 3000.0, 0.5))
            .unwrap();
        let mixer_right = builder
            .add_node(SummingMatrix::<f32, SvfOutput, Mono>::new())
            .unwrap();
        builder
            .connect_input(in_right, svf_right, SvfInput::AudioInput)
            .unwrap();
        builder
            .connect(
                svf_right,
                mixer_right,
                SvfOutput::Lowpass,
                SvfOutput::Lowpass,
            )
            .unwrap();
        builder
            .connect(
                svf_right,
                mixer_right,
                SvfOutput::Bandpass,
                SvfOutput::Bandpass,
            )
            .unwrap();
        builder
            .connect(
                svf_right,
                mixer_right,
                SvfOutput::Highpass,
                SvfOutput::Highpass,
            )
            .unwrap();
        builder
            .connect_output(mixer_right, out_right, Mono::Mono)
            .unwrap();

        for param in enum_iter::<PluginParams>() {
            let node = builder.add_param_node().unwrap();
            match param {
                PluginParams::SvfParam(param) => {
                    builder.connect_param(node, svf_left, param).unwrap();
                    builder.connect_param(node, svf_right, param).unwrap();
                }
                PluginParams::MixerParam(param) => {
                    builder.connect_param(node, mixer_left, param).unwrap();
                    builder.connect_param(node, mixer_right, param).unwrap();
                }
            }
        }

        Plugin {
            schedule: builder.compile(stream_data.block_size).unwrap(),
        }
    }
}

impl ClapModule for PluginConstructor {
    fn descriptor() -> PluginDescriptor {
        PluginDescriptor::new("dev.solarliner.clogbox.SvfMixer", "SVF Mixer")
            .with_description(
                "Clogbox example running the SVF filter and directly mixing its outputs",
            )
            .with_version(env!("CARGO_PKG_VERSION"))
            .with_features([features::AUDIO_EFFECT, features::FILTER, features::STEREO])
    }

    fn create(_: HostSharedHandle) -> Result<Self, PluginError> {
        Ok(Self)
    }
}

clap_module!(PluginConstructor);
