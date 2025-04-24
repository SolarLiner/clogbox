use crate::gen;
use clogbox_clap::params::{polynomial, DynMapping, MappingExt, ParamId, ParamInfoFlags};
use clogbox_clap::processor::{PluginCreateContext, PluginDsp};
use clogbox_enum::enum_map::{EnumMapArray, EnumMapRef};
use clogbox_enum::{Enum, Stereo};
use clogbox_math::interpolation::Linear;
use clogbox_math::root_eq::nr::NewtonRaphson;
use clogbox_math::{db_to_linear, linear_to_db};
use clogbox_module::sample::{SampleModule, SampleModuleWrapper, SampleProcessResult};
use clogbox_module::{module_wrapper, PrepareResult, Samplerate, StreamContext};
use clogbox_params::smoothers::{LinearSmoother, Smoother};
use num_traits::Float;
use std::fmt::Write;
use std::sync::LazyLock;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum Params {
    Cutoff,
    Drive,
}

impl ParamId for Params {
    fn text_to_value(&self, text: &str) -> Option<f32> {
        match self {
            Self::Cutoff => text.parse().ok(),
            Self::Drive => text.parse().ok().map(db_to_linear),
        }
    }

    fn default_value(&self) -> f32 {
        match self {
            Self::Cutoff => 1000.0,
            Self::Drive => 1.0,
        }
    }

    fn mapping(&self) -> DynMapping {
        static CUTOFF: LazyLock<DynMapping> = LazyLock::new(|| polynomial(20.0, 20e3, 2.0).into_dyn());
        static DRIVE: LazyLock<DynMapping> = LazyLock::new(|| polynomial(1.0, 100.0, 2.0).into_dyn());
        match self {
            Self::Cutoff => CUTOFF.clone(),
            Self::Drive => DRIVE.clone(),
        }
    }

    fn value_to_text(&self, f: &mut dyn Write, denormalized: f32) -> std::fmt::Result {
        match self {
            Self::Cutoff => write!(f, "{:.2} Hz", denormalized),
            Self::Drive => write!(f, "{:.2} dB", linear_to_db(denormalized)),
        }
    }

    fn flags(&self) -> ParamInfoFlags {
        ParamInfoFlags::IS_AUTOMATABLE
    }
}

pub struct SampleDsp {
    params: EnumMapArray<Params, LinearSmoother<f32>>,
    z: EnumMapArray<Stereo, f32>,
    wstep: f32,
}

impl SampleModule for SampleDsp {
    type Sample = f32;
    type AudioIn = Stereo;
    type AudioOut = Stereo;
    type Params = Params;

    fn prepare(&mut self, sample_rate: Samplerate) -> PrepareResult {
        self.z.as_slice_mut().fill(0.0);
        self.wstep = 0.5 * sample_rate.recip().value() as f32;
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
            smoother.set_target(x);
            smoother.next_value();
        }
        SampleProcessResult {
            tail: None,
            output: EnumMapArray::new(|ch| self.process_channel(ch, inputs[ch])),
        }
    }
}

impl SampleDsp {
    fn process_channel(&mut self, ch: Stereo, x: f32) -> f32 {
        use Params::*;
        let cutoff = self.params[Cutoff].current_value();
        let drive = self.params[Drive].current_value();
        let g = self.wstep * cutoff;
        let s = self.z[ch];
        let x = x * drive;

        const NR: NewtonRaphson<f32> = NewtonRaphson {
            tolerance: 1e-6,
            max_iterations: 100,
            over_relaxation: 1.0,
        };
        let eq = gen::EqU { x, g, s };
        let u = NR.solve(&eq, x.asinh()).value;
        if !u.is_finite() {
            return gen::y(x, 0.0, s, g) / drive;
        }
        let y = gen::y(g, s, u, x);
        let s = gen::s(g, s, u, x);
        self.z[ch] = s;
        y / drive.asinh()
    }
}

module_wrapper!(Dsp: SampleModuleWrapper<SampleDsp>);

impl PluginDsp for Dsp {
    type Plugin = super::NrClipper;

    fn create(context: PluginCreateContext<Self>) -> Self {
        let params = EnumMapArray::new(|p| context.params[p]);
        Self(SampleModuleWrapper::new(
            SampleDsp {
                z: EnumMapArray::new(|_| 0.0),
                params: EnumMapArray::new(|p| {
                    LinearSmoother::new(
                        Linear,
                        context.audio_config.sample_rate as _,
                        10e-3,
                        params[p],
                        params[p],
                    )
                }),
                wstep: 0.0,
            },
            params,
        ))
    }
}
