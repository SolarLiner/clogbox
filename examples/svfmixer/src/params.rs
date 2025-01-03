use clack_extensions::params::{ParamInfo, ParamInfoFlags, ParamInfoWriter};
use clack_plugin::prelude::*;
use clack_plugin::utils::Cookie;
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::Enum;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::{fmt, ops};

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Enum)]
pub enum ParamId {
    Cutoff,
    Resonance,
    Lowpass,
    Bandpass,
    Highpass,
}

impl ParamId {
    pub(crate) fn normalize(&self, value: f64) -> f64 {
        let range = self.range();
        let start = *range.start();
        let end = *range.end();
        let value = (value - start) / (end - start);
        match self {
            Self::Cutoff | Self::Resonance => value.powi(4),
            _ => value,
        }
    }

    pub(crate) fn clamp_value(&self, value: f64) -> f64 {
        let range = self.range();
        let start = *range.start();
        let end = *range.end();
        value.clamp(start, end)
    }

    /*    pub(crate) fn denormalize(&self, normalized: f64) -> f64 {
            let normalized = match self {
                Self::Cutoff | Self::Resonance => normalized.powf(1.0 / 4.0),
                _ => normalized,
            };
            let range = self.range();
            let start = *range.start();
            let end = *range.end();
            normalized * (end - start) + start
        }
    */
    pub(crate) fn write_param_info(&self, writer: &mut ParamInfoWriter) {
        let range = self.range();
        let name = self.name();
        let name = name.as_bytes();
        writer.set(&ParamInfo {
            id: ClapId::new(self.to_usize() as _),
            name,
            flags: self.flags(),
            default_value: self.default_value(),
            min_value: *range.start(),
            max_value: *range.end(),
            module: b"SvfMixer",
            cookie: Cookie::empty(),
        });
    }

    pub(crate) fn default_value(&self) -> f64 {
        match self {
            Self::Cutoff => 1000.0,
            Self::Resonance => 0.0,
            Self::Lowpass => 0.0,
            Self::Bandpass => 0.0,
            Self::Highpass => 0.0,
        }
    }

    pub(crate) fn display_value(&self, w: &mut impl fmt::Write, denormalized: f64) -> fmt::Result {
        match self {
            Self::Cutoff => write!(w, "{:4.1} Hz", denormalized),
            Self::Resonance => write!(w, "{:1.2} %", 100.0 * denormalized),
            Self::Lowpass | Self::Bandpass | Self::Highpass => {
                write!(w, "{:1.2} %", 100.0 * denormalized)
            }
        }
    }

    fn flags(&self) -> ParamInfoFlags {
        let flags = ParamInfoFlags::IS_AUTOMATABLE;
        flags
    }

    fn range(&self) -> ops::RangeInclusive<f64> {
        match self {
            Self::Cutoff => 20.0..=20000.0,
            Self::Resonance => 0.0..=2.0,
            Self::Lowpass | Self::Bandpass | Self::Highpass => -1.0..=1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Storage(Arc<EnumMapArray<ParamId, AtomicU32>>);

impl Default for Storage {
    fn default() -> Self {
        Self::new(EnumMapArray::new(|p: ParamId| p.default_value() as f32))
    }
}

impl Storage {
    pub fn new(params: EnumMapArray<ParamId, f32>) -> Self {
        Self(Arc::new(params.map(|_, v| AtomicU32::new(v.to_bits()))))
    }

    pub fn get_param(&self, param_id: ParamId) -> f32 {
        f32::from_bits(self.0[param_id].load(Ordering::SeqCst))
    }

    pub fn get_param_normalized(&self, param_id: ParamId) -> f64 {
        param_id.normalize(self.get_param(param_id) as f64)
    }

    pub fn set_param(&self, param_id: ParamId, value: f32) {
        self.0[param_id].store(value.to_bits(), Ordering::SeqCst);
    }

    /*    pub fn set_param_normalized(&self, param_id: ParamId, value: f32) {
            self.set_param(param_id, param_id.denormalize(value as _) as _);
        }
    */
}
