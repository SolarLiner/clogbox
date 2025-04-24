use clogbox_clap::params::{linear, polynomial, DynMapping, MappingExt, ParamId, ParamInfoFlags};
use clogbox_clap::processor::{PluginCreateContext, PluginDsp};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::Enum;
use clogbox_module::contrib::fundsp::FundspModule;
use clogbox_module::sample::SampleModuleWrapper;
use clogbox_module::{module_wrapper, Module};
use fundsp::prelude::*;
use std::fmt::Write;
use std::sync::LazyLock;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum Params {
    #[r#enum(display = "Delay Time")]
    DelayTime,
    Feedback,
    #[r#enum(display = "Dry/Wet")]
    DryWet,
}

impl ParamId for Params {
    fn text_to_value(&self, text: &str) -> Option<f32> {
        let parsed = text.parse::<f32>().ok()?;
        Some(if matches!(self, Params::Feedback | Params::DryWet) {
            parsed * 100.0
        } else {
            parsed
        })
    }

    fn default_value(&self) -> f32 {
        match self {
            Params::DelayTime => 3.0,
            Params::Feedback => 0.707,
            Params::DryWet => 0.5,
        }
    }

    fn mapping(&self) -> DynMapping {
        static DELAYTIME: LazyLock<DynMapping> = LazyLock::new(|| polynomial(0.01, 10.0, 2.5).into_dyn());
        static FEEDBACK: LazyLock<DynMapping> = LazyLock::new(|| polynomial(0.0, 1.5, 0.5).into_dyn());
        static DRYWET: LazyLock<DynMapping> = LazyLock::new(|| linear(0.0, 1.0).into_dyn());

        match self {
            Params::DelayTime => DELAYTIME.clone(),
            Params::Feedback => FEEDBACK.clone(),
            Params::DryWet => DRYWET.clone(),
        }
    }

    fn value_to_text(&self, f: &mut dyn Write, denormalized: f32) -> std::fmt::Result {
        match self {
            Params::DelayTime => {
                if denormalized > 1.0 {
                    write!(f, "{denormalized:.2} s")
                } else {
                    write!(f, "{:.2} ms", denormalized * 1000.0)
                }
            }
            Params::Feedback | Params::DryWet => write!(f, "{:.2} %", 100.0 * denormalized),
        }
    }

    fn flags(&self) -> ParamInfoFlags {
        ParamInfoFlags::IS_AUTOMATABLE
    }
}

module_wrapper!(Dsp: SampleModuleWrapper<FundspModule<Unit<U2, U2>, Params>>);

impl PluginDsp for Dsp {
    type Plugin = super::FundspPlugin;

    fn create(context: PluginCreateContext<Self>) -> Self {
        let default = EnumMapArray::new(|p: Params| p.default_value());
        let dsp = FundspModule::create(|mut params| {
            for (param, shared) in params.iter_mut() {
                shared.set_value(default[param])
            }
            let delay = || {
                (pass() | var(&params[Params::DelayTime]))
                    >> (tap(0.0, 10.0) * var(&params[Params::Feedback]))
                    >> shape(Tanh(1.0))
            };
            let mono =
                || (pass() * (1.0 - var(&params[Params::DryWet]))) & (var(&params[Params::DryWet]) * feedback(delay()));
            let node = mono() | mono();
            An(Unit::new(Box::new(node)))
        });
        let mut module = SampleModuleWrapper::new(dsp, default);
        module.prepare(context.audio_config.sample_rate.into(), 1);
        Self(module)
    }
}
