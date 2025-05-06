use clogbox_clap::params::{int, polynomial, DynMapping, MappingExt, ParamId, ParamInfoFlags};
use clogbox_clap::processor::{PluginCreateContext, PluginDsp};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{enum_iter, Empty, Enum, Stereo};
use clogbox_module::modules::env_follower;
use clogbox_module::modules::env_follower::EnvFollower;
use clogbox_module::modules::extract::{BufferSize, ExtractAudio};
use clogbox_module::sample::{SampleModule, SampleModuleWrapper};
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
use std::fmt::Write;
use std::sync::{Arc, LazyLock};
use clogbox_clap::main_thread::Plugin;
use clogbox_module::context::{OwnedProcessContext, ProcessContext};
use crate::PluginData;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum AudioOut {
    Output(Stereo),
    Envelope(Stereo),
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum Params {
    Envelope(env_follower::Params),
}

impl ParamId for Params {
    fn text_to_value(&self, text: &str) -> Option<f32> {
        let parsed = text.parse::<f32>().ok()?;
        match self {
            Params::Envelope(env_follower::Params::FollowMode) => None,
            _ => Some(parsed),
        }
    }

    fn default_value(&self) -> f32 {
        match self {
            Self::Envelope(env_follower::Params::FollowMode) => 0.0,
            Params::Envelope(env_follower::Params::Attack) => 0.04,
            Params::Envelope(env_follower::Params::Release) => 0.15,
            Params::Envelope(env_follower::Params::RmsTime) => 0.1,
        }
    }

    fn mapping(&self) -> DynMapping {
        static TIME: LazyLock<DynMapping> = LazyLock::new(|| polynomial(0.01, 10.0, 2.5).into_dyn());
        static FOLLOW_MODE: LazyLock<DynMapping> = LazyLock::new(|| int(0, 1).into_dyn());

        match self {
            Self::Envelope(
                env_follower::Params::Attack | env_follower::Params::Release | env_follower::Params::RmsTime,
            ) => TIME.clone(),
            Self::Envelope(env_follower::Params::FollowMode) => FOLLOW_MODE.clone(),
        }
    }

    fn value_to_text(&self, f: &mut dyn Write, denormalized: f32) -> std::fmt::Result {
        match self {
            Self::Envelope(env_follower::Params::FollowMode) if denormalized < 0.5 => write!(f, "Peak"),
            Self::Envelope(env_follower::Params::FollowMode) => write!(f, "RMS"),
            Self::Envelope(_) => write!(f, "{:.2} s", denormalized),
        }
    }

    fn flags(&self) -> ParamInfoFlags {
        ParamInfoFlags::IS_AUTOMATABLE
    }
}

pub struct Dsp {
    env_context: OwnedProcessContext<SampleModuleWrapper<EnvFollower<f32, Stereo>>>,
    env_follower: SampleModuleWrapper<EnvFollower<f32, Stereo>>,
    extract_context: OwnedProcessContext<ExtractAudio<f32, Stereo>>,
    extract_audio: ExtractAudio<f32, Stereo>,
    pub shared_data: PluginData,
}

impl Module for Dsp {
    type Sample = f32;
    type AudioIn = Stereo;
    type AudioOut = AudioOut;
    type ParamsIn = Params;
    type ParamsOut = Empty;
    type NoteIn = Empty;
    type NoteOut = Empty;

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
        self.extract_context = OwnedProcessContext::new(block_size, 0);
        self.env_follower.prepare(sample_rate, block_size);
        self.extract_audio.prepare(sample_rate, block_size);
        self.shared_data.cb.store(Arc::new(self.extract_audio.circular_buffer()));
        PrepareResult { latency: 0.0 }
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        self.env_context.audio_in.copy_from_input(context.audio_in);
        for (param, slice) in self.env_context.params_in.iter_mut() {
            slice.clear();
            for event in context.params_in[Params::Envelope(param)].iter() {
                slice.push(event.timestamp, event.data);
            }
        }
        self.env_context.process_with(context.stream_context, |ctx| self.env_follower.process(ctx));
        
        self.extract_context.audio_in.copy_from_input(&self.env_context.audio_out);
        let r2 = self.extract_context.process_with(context.stream_context, |ctx| self.extract_audio.process(ctx));
        
        for param in enum_iter::<Self::AudioOut>() {
            let slice = &mut context.audio_out[param];
            match param {
                AudioOut::Output(s) => {
                    slice.copy_from_slice(&context.audio_in[s]);
                }
                AudioOut::Envelope(s) => {
                    slice.copy_from_slice(&self.env_context.audio_out[s]);
                }
            }
        }
        
        r2
    }
}

impl PluginDsp for Dsp {
    type Plugin = super::EnvFollowerPlugin;

    fn create(context: PluginCreateContext<Self>, shared_data: &<Self::Plugin as Plugin>::SharedData) -> Self {
        let extract_audio = ExtractAudio::new(BufferSize::Seconds(0.01));
        shared_data.cb.store(Arc::new(extract_audio.circular_buffer()));
        Self {
            env_context: OwnedProcessContext::new(0, 0),
            env_follower: SampleModuleWrapper::new(
                EnvFollower::new(
                    context.params[Params::Envelope(env_follower::Params::Attack)],
                    context.params[Params::Envelope(env_follower::Params::Release)],
                    if context.params[Params::Envelope(env_follower::Params::FollowMode)] < 0.5 {
                        env_follower::FollowMode::Peak
                    } else {
                        env_follower::FollowMode::Rms(
                            context.params[Params::Envelope(env_follower::Params::RmsTime)] as _,
                        )
                    },
                ),
                EnumMapArray::new(|e| context.params[Params::Envelope(e)]),
            ),
            extract_context: OwnedProcessContext::new(0, 0),
            extract_audio,
            shared_data: shared_data.clone(),
        }
    }
}
