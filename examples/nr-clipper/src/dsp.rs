use crate::{gen, SharedData};
use clogbox_clap::dsp::PluginCreateContext;
use clogbox_clap::dsp::PluginDsp;
use clogbox_clap::params::{decibel, frequency, int, linear, DynMapping, MappingExt, ParamId};
use clogbox_enum::enum_map::{EnumMapArray, EnumMapRef};
use clogbox_enum::{enum_iter, Enum, Mono, Stereo};
use clogbox_filters::DcBlocker;
use clogbox_math::interpolation::Linear;
use clogbox_math::root_eq::nr::NewtonRaphson;
use clogbox_math::{db_to_linear, linear_to_db};
use clogbox_module::context::StreamContext;
use clogbox_module::modules::env_follower::EnvFollower;
use clogbox_module::sample::{SampleModule, SampleModuleWrapper, SampleProcessResult};
use clogbox_module::{module_wrapper, PrepareResult, Samplerate};
use clogbox_params::smoothers::{LinearSmoother, Smoother};
use num_traits::Float;
use std::f32::consts::PI;
use std::fmt::Write;
use std::sync::atomic::Ordering;
use std::sync::LazyLock;

pub const NUM_STAGES: usize = 8;
const LED_ENV_ATTACK: f32 = 16e-3;
const LED_ENV_DECAY: f32 = 50e-3;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum Params {
    Cutoff,
    Drive,
    Bias,
    NumStages,
}

impl ParamId for Params {
    fn text_to_value(&self, text: &str) -> Option<f32> {
        match self {
            Self::Cutoff => text.parse().ok(),
            Self::Drive => text.parse().ok().map(db_to_linear),
            Self::Bias => text.parse().ok(),
            Self::NumStages => text.parse().ok().map(|f: f32| f.round().clamp(1.0, NUM_STAGES as f32)),
        }
    }

    fn default_value(&self) -> f32 {
        match self {
            Self::Cutoff => 1000.0,
            Self::Drive => 1.0,
            Self::Bias => 0.0,
            Self::NumStages => 1.0,
        }
    }

    fn mapping(&self) -> DynMapping {
        static CUTOFF: LazyLock<DynMapping> = LazyLock::new(|| frequency(20.0, 20e3).into_dyn());
        static DRIVE: LazyLock<DynMapping> = LazyLock::new(|| decibel(0.0, 60.0).into_dyn());
        static BIAS: LazyLock<DynMapping> = LazyLock::new(|| linear(-100.0, 100.0).into_dyn());
        static NUM_STAGES: LazyLock<DynMapping> = LazyLock::new(|| int(1, self::NUM_STAGES as i32).into_dyn());
        match self {
            Self::Cutoff => CUTOFF.clone(),
            Self::Drive => DRIVE.clone(),
            Self::Bias => BIAS.clone(),
            Self::NumStages => NUM_STAGES.clone(),
        }
    }

    fn value_to_text(&self, f: &mut dyn Write, denormalized: f32) -> std::fmt::Result {
        match self {
            Self::Cutoff => write!(f, "{:.2} Hz", denormalized),
            Self::Drive => write!(f, "{:.2} dB", linear_to_db(denormalized)),
            Self::Bias => write!(f, "{:2.1} %", denormalized),
            Self::NumStages => write!(
                f,
                "{:.0} stage{}",
                denormalized.round(),
                if denormalized.round() == 1.0 { "" } else { "s" }
            ),
        }
    }
}

#[derive(Clone)]
pub struct Stage {
    last_s: EnumMapArray<Stereo, f32>,
    last_u: EnumMapArray<Stereo, f32>,
    wstep: f32,
    led_env_follow: EnvFollower<f32>,
}

impl Stage {
    fn prepare(&mut self, sample_rate: Samplerate) -> PrepareResult {
        self.last_s.as_slice_mut().fill(0.0);
        self.wstep = PI * sample_rate.recip().value() as f32;
        self.led_env_follow.prepare(sample_rate);
        PrepareResult { latency: 0.0 }
    }

    fn process_channel(&mut self, ch: Stereo, params: EnumMapRef<Params, f32>, x: f32) -> f32 {
        use Params::*;
        let cutoff = params[Cutoff];
        let drive = params[Drive];
        let bias = params[Bias];
        let g = self.wstep * cutoff;
        let s = self.last_s[ch];

        const NR: NewtonRaphson<f32> = NewtonRaphson::new(1000, 1e-6);
        let eq = gen::Equation {
            g,
            s,
            x,
            k_drive: drive,
            k_bias: bias,
        };
        let mut u = NR.solve(&eq, self.last_u[ch]).value;
        if !u.is_finite() {
            u = self.last_u[ch];
        }
        let y = gen::y(g, x, s, u);
        let s = gen::s(g, x, y, u);

        self.last_u[ch] = u;
        self.last_s[ch] = s;
        y * drive.sqrt()
    }

    fn process_sample(
        &mut self,
        inputs: EnumMapArray<Stereo, f32>,
        params: EnumMapRef<Params, f32>,
    ) -> (EnumMapArray<Stereo, f32>, f32) {
        use Params::*;
        let output = EnumMapArray::new(|ch| self.process_channel(ch, params, inputs[ch]));
        let diff = enum_iter::<Stereo>()
            .map(|ch| params[Drive] * self.last_u[ch])
            .map(|x| x.asinh() - x)
            .sum::<f32>()
            / 2.0;
        let env = self
            .led_env_follow
            .process_follower(EnumMapArray::from_std_array([diff.powi(2)]));
        (output, env[Mono])
    }
}

pub struct SampleDsp {
    stages: [Stage; NUM_STAGES],
    dc_blocker: EnumMapArray<Stereo, DcBlocker<f32>>,
    num_active: usize,
    params: EnumMapArray<Params, LinearSmoother<f32>>,
    pub shared_data: SharedData,
}

impl SampleModule for SampleDsp {
    type Sample = f32;
    type AudioIn = Stereo;
    type AudioOut = Stereo;
    type Params = Params;

    fn prepare(&mut self, sample_rate: Samplerate) -> PrepareResult {
        for stage in &mut self.stages {
            stage.prepare(sample_rate);
        }
        PrepareResult { latency: 0.0 }
    }

    fn process(
        &mut self,
        _: &StreamContext,
        inputs: EnumMapArray<Self::AudioIn, Self::Sample>,
        params: EnumMapRef<Self::Params, f32>,
    ) -> SampleProcessResult<Self::AudioOut, Self::Sample> {
        for (param, smoother) in self.params.iter_mut() {
            let x = params[param];
            let x = if matches!(param, Params::Bias) {
                0.45 * x / 100.0
            } else {
                x
            };
            smoother.set_target(x);
        }

        self.num_active = params[Params::NumStages] as usize;

        let params = EnumMapArray::new(|e: Params| self.params[e].next_value());
        let zero = EnumMapArray::new(|_| 0.0);
        let mut output = inputs.clone();
        for (i, stage) in self.stages.iter_mut().enumerate() {
            let led = if i < self.num_active {
                let (out, led) = stage.process_sample(output, params.to_ref());
                output = out;
                led
            } else {
                // Still process (for LEDs)
                stage.process_sample(zero, params.to_ref()).1
            };
            self.shared_data.drive_led[i].store(led, Ordering::Relaxed);
        }
        for ch in enum_iter::<Stereo>() {
            output[ch] = self.dc_blocker[ch].next_sample(output[ch]);
        }
        SampleProcessResult { tail: None, output }
    }
}

module_wrapper!(Dsp: SampleModuleWrapper<SampleDsp>);

impl PluginDsp for Dsp {
    type Plugin = super::NrClipper;

    fn create(context: PluginCreateContext<Self>, shared_data: &SharedData) -> Self {
        let samplerate = context.audio_config.sample_rate as f32;
        let params = EnumMapArray::new(|p| context.params[p]);
        let num_active = params[Params::NumStages] as usize;
        let smoothers = EnumMapArray::new(|p| LinearSmoother::new(Linear, samplerate, 10e-3, params[p], params[p]));
        let stage = Stage {
            last_s: EnumMapArray::new(|_| 0.0),
            last_u: EnumMapArray::new(|_| 0.0),
            wstep: 0.0,
            led_env_follow: EnvFollower::new(LED_ENV_ATTACK, LED_ENV_DECAY),
        };
        Self(SampleModuleWrapper::new(
            SampleDsp {
                params: smoothers,
                num_active,
                stages: std::array::from_fn(|_| stage.clone()),
                dc_blocker: EnumMapArray::new(|_| DcBlocker::new(samplerate)),
                shared_data: shared_data.clone(),
            },
            params,
        ))
    }
}
