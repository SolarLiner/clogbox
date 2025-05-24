use crate::SharedData;
use clogbox_clap::main_thread::Plugin;
use clogbox_clap::params;
use clogbox_clap::params::{decibel, enum_, linear, polynomial, DynMapping, Linear, Mapping, MappingExt, ParamId};
use clogbox_clap::processor::{PluginCreateContext, PluginDsp};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{count, enum_iter, Empty, Enum, Mono, Stereo};
use clogbox_math::{db_to_linear, linear_to_db};
use clogbox_module::context::{AudioStorage, OwnedProcessContext, ProcessContext};
use clogbox_module::eventbuffer::Timestamped;
use clogbox_module::modules::env_follower;
use clogbox_module::modules::env_follower::EnvFollower;
use clogbox_module::modules::extract::ExtractAudio;
use clogbox_module::sample::SampleModuleWrapper;
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
use clogbox_params::smoothers::{ExpSmoother, InterpSmoother, LinearSmoother, Smoother};
use std::fmt::Write;
use std::ops;
use std::ops::Range;
use std::sync::{Arc, LazyLock};

#[derive(Debug, Copy, Clone)]
struct Recip(params::Range<Linear>);

impl Mapping for Recip {
    fn normalize(&self, value: f32) -> f32 {
        self.0.normalize(value.recip())
    }

    fn denormalize(&self, value: f32) -> f32 {
        self.0.denormalize(value).recip()
    }

    fn range(&self) -> Range<f32> {
        self.0.range()
    }
}

fn recip(min: f32, max: f32) -> Recip {
    Recip(linear(min, max))
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum SidechainMode {
    Internal,
    External,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum AudioIn {
    Input(Stereo),
    Sidechain(Stereo),
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum Params {
    Envelope(env_follower::Params),
    Threshold,
    Ratio,
    Makeup,
    StereoLink,
    SidechainMode,
}

impl ParamId for Params {
    fn text_to_value(&self, text: &str) -> Option<f32> {
        let parsed = text.parse::<f32>().ok()?;
        Some(parsed)
    }

    fn default_value(&self) -> f32 {
        match self {
            Params::Envelope(env_follower::Params::Attack) => 0.04,
            Params::Envelope(env_follower::Params::Release) => 0.15,
            Params::Threshold => -18.0,
            Params::Ratio => 4f32.recip(),
            Params::Makeup => 1.0,
            Params::StereoLink => 1.0,
            Params::SidechainMode => 0.0,
        }
    }

    fn mapping(&self) -> DynMapping {
        static TIME: LazyLock<DynMapping> = LazyLock::new(|| polynomial(1e-6, 1.0, 2.5f32.ln()).into_dyn());
        static THRESHOLD: LazyLock<DynMapping> = LazyLock::new(|| linear(-72.0, 0.0).into_dyn());
        static RATIO: LazyLock<DynMapping> = LazyLock::new(|| recip(1.0, 20.0).into_dyn());
        static MAKEUP: LazyLock<DynMapping> = LazyLock::new(|| decibel(0.0, 24.0).into_dyn());
        static SIDECHAIN: LazyLock<DynMapping> = LazyLock::new(|| enum_::<SidechainMode>().into_dyn());
        static STEREO_LINK: LazyLock<DynMapping> = LazyLock::new(|| linear(0.0, 1.0).into_dyn());

        match self {
            Self::Envelope(env_follower::Params::Attack | env_follower::Params::Release) => TIME.clone(),
            Self::Threshold => THRESHOLD.clone(),
            Self::Ratio => RATIO.clone(),
            Self::Makeup => MAKEUP.clone(),
            Self::StereoLink => STEREO_LINK.clone(),
            Self::SidechainMode => SIDECHAIN.clone(),
        }
    }

    fn value_to_text(&self, f: &mut dyn Write, denormalized: f32) -> std::fmt::Result {
        match self {
            Self::Envelope(_) => {
                if denormalized < 0.9 {
                    write!(f, "{:.2} ms", 1e3 * denormalized)
                } else {
                    write!(f, "{:.2} s", denormalized)
                }
            }
            Self::Threshold => write!(f, "{:.2} dB", denormalized),
            Self::Ratio => write!(f, "{:1.2}:1", denormalized.recip()),
            Self::Makeup => write!(f, "{:.2} dB", linear_to_db(denormalized)),
            Self::StereoLink => write!(f, "{:2} %", 100. * denormalized),
            Self::SidechainMode => write!(f, "{}", SidechainMode::from_usize(denormalized.round() as _).name()),
        }
    }

    fn discrete(&self) -> Option<usize> {
        match self {
            Self::SidechainMode => Some(count::<SidechainMode>()),
            _ => None,
        }
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
enum SmoothedParams {
    Theshold,
    Ratio,
    Link,
    SidechainSwitch,
    MakeupGain,
}

impl SmoothedParams {
    pub(crate) fn to_dsp_param(&self) -> Params {
        match self {
            SmoothedParams::Theshold => Params::Threshold,
            SmoothedParams::Ratio => Params::Ratio,
            SmoothedParams::Link => Params::StereoLink,
            SmoothedParams::SidechainSwitch => Params::SidechainMode,
            SmoothedParams::MakeupGain => Params::Makeup,
        }
    }
}

fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

fn compressor_gain(ratio_recip: f32, threshold_db: f32, input: f32) -> f32 {
    let x = linear_to_db(input);
    let curve_db = lerp(1.0 - ratio_recip, x, (x - threshold_db).min(0.0) + threshold_db);
    let gain_db = curve_db - x;
    db_to_linear(gain_db)
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
    let k = k * 6.0;
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - (h * h * h) * k * (1.0 / 6.0)
}

#[inline]
fn smax(a: f32, b: f32, k: f32) -> f32 {
    -smin(-a, -b, k)
}

fn soft_clipper(x: f32, max_gain: f32) -> f32 {
    const K: f32 = 0.3;
    smax(-max_gain, smin(max_gain, x, K), K)
}

pub struct Dsp {
    env_context: OwnedProcessContext<SampleModuleWrapper<EnvFollower<f32, Stereo>>>,
    env_follower: SampleModuleWrapper<EnvFollower<f32, Stereo>>,
    extract_context: OwnedProcessContext<ExtractAudio<f32, Stereo>>,
    extract_audio: ExtractAudio<f32, Stereo>,
    smoothers: EnumMapArray<SmoothedParams, ExpSmoother<f32>>,
    param_signals: AudioStorage<SmoothedParams, f32>,
    pub shared_data: SharedData,
}

impl Dsp {
    fn compute_smoothed_signals(&mut self, context: &ProcessContext<Self>) {
        let block_size = context.stream_context.block_size;
        for param in enum_iter::<SmoothedParams>() {
            for i in 0..block_size {
                if let Some(&Timestamped { data, .. }) = context.params_in[param.to_dsp_param()].at(i) {
                    self.smoothers[param].set_target(data);
                }
                self.param_signals[param][i] = self.smoothers[param].next_value();
            }
        }
    }

    fn process_env_follower(&mut self, context: &ProcessContext<Dsp>) {
        let block_size = context.stream_context.block_size;
        let mix = &self.param_signals[SmoothedParams::SidechainSwitch];
        for ch in enum_iter::<Stereo>() {
            let env_input = &mut self.env_context.audio_in[ch];
            let input = &context.audio_in[AudioIn::Input(ch)];
            let sidechain = &context.audio_in[AudioIn::Sidechain(ch)];
            for i in 0..block_size {
                env_input[i] = input[i] + (sidechain[i] - input[i]) * mix[i];
            }
        }
        for (param, slice) in self.env_context.params_in.iter_mut() {
            slice.clear();
            for event in context.params_in[Params::Envelope(param)].iter() {
                slice.push(event.timestamp, event.data);
            }
        }
        self.env_context
            .process_with(context.stream_context, |ctx| self.env_follower.process(ctx));
    }

    fn compute_compressor_gain(&mut self, context: &ProcessContext<Self>) {
        let link = &self.param_signals[SmoothedParams::Link];
        let threshold = &self.param_signals[SmoothedParams::Theshold];
        let ratio = &self.param_signals[SmoothedParams::Ratio];
        let block_size = context.stream_context.block_size;
        for ch in enum_iter::<Stereo>() {
            let env_l = &self.env_context.audio_out[Stereo::Left];
            let env_r = &self.env_context.audio_out[Stereo::Right];
            let out = &mut self.extract_context.audio_in[ch];
            for i in 0..block_size {
                let mix = match ch {
                    Stereo::Left => 0.5 * link[i],
                    Stereo::Right => 1.0 - 0.5 * link[i],
                };
                let env = env_l[i] + (env_r[i] - env_l[i]) * mix;
                out[i] = compressor_gain(ratio[i], threshold[i], env);
            }
        }
    }

    fn apply_gain_reduction(&mut self, context: &mut ProcessContext<Self>) {
        let input = &*context.audio_in;
        let output = &mut *context.audio_out;
        let block_size = context.stream_context.block_size;
        let makeup = &self.param_signals[SmoothedParams::MakeupGain];
        for ch in enum_iter::<Stereo>() {
            let amp = &self.extract_context.audio_in[ch];
            let inp = &input[AudioIn::Input(ch)];
            let out = &mut output[ch];
            for i in 0..block_size {
                const MAX_GAIN: f32 = 0.988553094657; // -0.1 dB
                out[i] = inp[i] * amp[i] * makeup[i];
                out[i] = soft_clipper(out[i], MAX_GAIN);
            }
        }
    }

    fn process_extract_audio(&mut self, context: &ProcessContext<Dsp>) {
        self.extract_context
            .process_with(context.stream_context, |ctx| self.extract_audio.process(ctx));
    }
}

impl Module for Dsp {
    type Sample = f32;
    type AudioIn = AudioIn;
    type AudioOut = Stereo;
    type ParamsIn = Params;
    type ParamsOut = Empty;
    type NoteIn = Empty;
    type NoteOut = Empty;

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
        self.env_context = OwnedProcessContext::new(block_size, 128);
        self.extract_context = OwnedProcessContext::new(block_size, 128);
        self.env_follower.prepare(sample_rate, block_size);
        let (tx, rx) = fixed_ringbuf::create(sample_rate.value() as usize);
        self.extract_audio.set_tx(tx);
        self.shared_data.cb.store(Arc::new(Some(rx)));
        self.extract_audio.prepare(sample_rate, block_size);
        self.shared_data
            .samplerate
            .store(sample_rate.value() as _, std::sync::atomic::Ordering::Relaxed);
        self.param_signals = AudioStorage::zeroed(block_size);
        for ch in enum_iter::<SmoothedParams>() {
            self.smoothers[ch].set_samplerate(sample_rate.value() as _);
        }
        PrepareResult { latency: 0.0 }
    }

    fn process(&mut self, mut context: ProcessContext<Self>) -> ProcessResult {
        self.compute_smoothed_signals(&context);
        self.process_env_follower(&context);
        self.compute_compressor_gain(&context);
        self.apply_gain_reduction(&mut context);
        self.process_extract_audio(&context);
        ProcessResult { tail: None }
    }
}

impl PluginDsp for Dsp {
    type Plugin = super::Compressor;

    fn create(context: PluginCreateContext<Self>, shared_data: &<Self::Plugin as Plugin>::SharedData) -> Self {
        let extract_audio = ExtractAudio::CONST_NEW;
        let sample_rate = context.audio_config.sample_rate as _;
        Self {
            env_context: OwnedProcessContext::new(0, 0),
            env_follower: SampleModuleWrapper::new(
                EnvFollower::new(
                    context.params[Params::Envelope(env_follower::Params::Attack)],
                    context.params[Params::Envelope(env_follower::Params::Release)],
                ),
                EnumMapArray::new(|e| context.params[Params::Envelope(e)]),
            ),
            extract_context: OwnedProcessContext::new(0, 0),
            extract_audio,
            smoothers: EnumMapArray::new(|p: SmoothedParams| {
                ExpSmoother::new(
                    sample_rate,
                    10e-3,
                    context.params[p.to_dsp_param()],
                    context.params[p.to_dsp_param()],
                )
            }),
            param_signals: AudioStorage::zeroed(0),
            shared_data: shared_data.clone(),
        }
    }
}
