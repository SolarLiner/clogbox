use clogbox_clap::params::{decibel, enum_, frequency, polynomial_raw, DynMapping, MappingExt, ParamId};
use clogbox_enum::{count, Enum};
use clogbox_filters::svf::FilterType;
use std::fmt;
use std::fmt::Write;
use std::sync::LazyLock;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Enum)]
pub enum Param {
    Cutoff,
    Resonance,
    Drive,
    Gain,
    #[r#enum(display = "Filter Type")]
    FilterType,
}

impl ParamId for Param {
    fn text_to_value(&self, text: &str) -> Option<f32> {
        text.parse().ok()
    }

    fn default_value(&self) -> f32 {
        match self {
            Param::Cutoff => 1000.0,
            Param::Resonance => 0.5,
            Param::Drive => 1.0,
            Param::Gain => 1.0,
            Param::FilterType => FilterType::Bypass.to_usize() as _,
        }
    }

    fn mapping(&self) -> DynMapping {
        static CUTOFF_MAPPING: LazyLock<DynMapping> = LazyLock::new(|| frequency(20., 20e3).into_dyn());
        static RESO_MAPPING: LazyLock<DynMapping> = LazyLock::new(|| polynomial_raw(0.0, 1.5, 0.5).into_dyn());
        static GAIN_MAPPING: LazyLock<DynMapping> = LazyLock::new(|| decibel(-20.0, 20.0).into_dyn());
        static DRIVE_MAPPING: LazyLock<DynMapping> = LazyLock::new(|| decibel(-20.0, 40.0).into_dyn());
        static MIXING_TYPE: LazyLock<DynMapping> = LazyLock::new(|| enum_::<FilterType>().into_dyn());

        match self {
            Self::Cutoff => CUTOFF_MAPPING.clone(),
            Self::Resonance => RESO_MAPPING.clone(),
            Self::Drive => DRIVE_MAPPING.clone(),
            Self::Gain => GAIN_MAPPING.clone(),
            Self::FilterType => MIXING_TYPE.clone(),
        }
    }

    fn value_to_text(&self, f: &mut dyn Write, denormalized: f32) -> fmt::Result {
        match self {
            Self::Cutoff => write!(f, "{:4.1} Hz", denormalized),
            Self::Resonance => write!(f, "{:1.2} %", 100.0 * denormalized),
            Self::Drive | Self::Gain => write!(f, "{:1.2} dB", 20.0 * denormalized.abs().log10()),
            Self::FilterType => {
                let e = FilterType::from_usize(denormalized as _);
                write!(f, "{}", e.name())
            }
        }
    }

    fn discrete(&self) -> Option<usize> {
        if matches!(self, Self::FilterType) {
            Some(count::<FilterType>())
        } else {
            None
        }
    }
}
