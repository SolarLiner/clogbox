use clogbox_clap::params::{linear, polynomial_raw, DynMapping, MappingExt, ParamId, ParamInfoFlags};
use clogbox_enum::Enum;
use std::fmt;
use std::fmt::Write;
use std::sync::LazyLock;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Enum)]
pub enum Param {
    Cutoff,
    Resonance,
    Lowpass,
    Bandpass,
    Highpass,
}

impl ParamId for Param {
    fn text_to_value(&self, text: &str) -> Option<f32> {
        text.parse().ok()
    }

    fn default_value(&self) -> f32 {
        match self {
            Param::Cutoff => 1000.0,
            Param::Resonance => 0.5,
            Param::Lowpass => 1.0,
            Param::Bandpass => 0.0,
            Param::Highpass => 0.0,
        }
    }

    fn mapping(&self) -> DynMapping {
        static CUTOFF_MAPPING: LazyLock<DynMapping> = LazyLock::new(|| polynomial_raw(20.0, 20e3, 4.0).into_dyn());
        static RESO_MAPPING: LazyLock<DynMapping> = LazyLock::new(|| polynomial_raw(0.0, 1.0, 0.5).into_dyn());
        static ATTENUVERTER_MAPPING: LazyLock<DynMapping> = LazyLock::new(|| linear(-1.0, 1.0).into_dyn());

        match self {
            Self::Cutoff => CUTOFF_MAPPING.clone(),
            Self::Resonance => RESO_MAPPING.clone(),
            Self::Lowpass | Self::Bandpass | Self::Highpass => ATTENUVERTER_MAPPING.clone(),
        }
    }

    fn value_to_text(&self, f: &mut dyn Write, denormalized: f32) -> fmt::Result {
        match self {
            Self::Cutoff => write!(f, "{:4.1} Hz", denormalized),
            Self::Resonance => write!(f, "{:1.2} %", 100.0 * denormalized),
            Self::Lowpass | Self::Bandpass | Self::Highpass => {
                let db = 20.0 * denormalized.abs().log10();
                write!(f, "{:+2.2} dB", db)?;
                if denormalized < 0.0 {
                    write!(f, " (inv)")?;
                }
                Ok(())
            }
        }
    }

    fn flags(&self) -> ParamInfoFlags {
        ParamInfoFlags::IS_AUTOMATABLE
    }
}
