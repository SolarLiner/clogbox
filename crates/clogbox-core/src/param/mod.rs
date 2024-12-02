use crate::r#enum::{count, Empty, Enum, Sequential};
use bitflags::bitflags;
use std::fmt::Formatter;
use std::{fmt, ops};
use thiserror::Error;
use typenum::Unsigned;

pub mod events;
pub mod smoother;
pub mod container;

#[derive(Debug, Error)]
#[error("Invalid range for normalized parameter: {0}")]
#[repr(transparent)]
pub struct InvalidRange(pub f32);

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
#[repr(transparent)]
pub struct Normalized(f32);

impl fmt::Display for Normalized {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        <f32 as fmt::Display>::fmt(&self.0, f)
    }
}

impl ops::Deref for Normalized {
    type Target = f32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<f32> for Normalized {
    fn as_ref(&self) -> &f32 {
        &self.0
    }
}

impl Normalized {
    pub const ZERO: Self = Self(0.0);
    pub const ONE: Self = Self(1.0);
    pub const HALF: Self = Self(0.5);

    pub const fn new(value: f32) -> Option<Self> {
        if value < 0.0 || value > 1.0 {
            None
        } else {
            Some(Self(value))
        }
    }

    pub const unsafe fn new_unchecked(value: f32) -> Self {
        Self(value)
    }

    pub const fn from_ref(value: &f32) -> Option<&Self> {
        if *value < 0.0 || *value > 1.0 {
            // Safety: Self is repr(transparent) with f32
            Some(unsafe { std::mem::transmute::<&f32, &Self>(value) })
        } else {
            None
        }
    }

    pub const unsafe fn from_ref_unchecked(value: &f32) -> &Self {
        // Safety: Self is repr(transparent) with f32
        unsafe { std::mem::transmute::<&f32, &Self>(value) }
    }

    pub const fn into_inner(self) -> f32 {
        self.0
    }

    pub const unsafe fn set_unchecked(&mut self, value: f32) {
        self.0 = value;
    }

    pub const fn set(&mut self, value: f32) -> Result<(), InvalidRange> {
        if value < 0.0 || value > 1.0 {
            Err(InvalidRange(value))
        } else {
            self.0 = value;
            Ok(())
        }
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, Eq, Hash, Ord, PartialOrd, PartialEq)]
    pub struct ParamFlags: u16 {
        const HIDDEN = 1 << 0;
        const MODULABLE = 1 << 1;
        const AUTOMATABLE = 1 << 2;
        const MODULABLE_AUTOMATABLE = Self::MODULABLE.bits() | Self::AUTOMATABLE.bits();
        const AUTOMATABLE_MODULABLE = Self::MODULABLE_AUTOMATABLE.bits();
    }
}

impl Default for ParamFlags {
    fn default() -> Self {
        Self::CONST_DEFAULT
    }
}

impl ParamFlags {
    pub const CONST_DEFAULT: Self = Self::MODULABLE_AUTOMATABLE;
}

#[derive(Debug, Clone)]
pub struct ParamMetadata {
    pub range: ops::RangeInclusive<f32>,
    pub default: Normalized,
    pub flags: ParamFlags,
}

impl Default for ParamMetadata {
    fn default() -> Self {
        Self::CONST_DEFAULT
    }
}

impl ParamMetadata {
    pub const CONST_DEFAULT: Self = Self {
        range: 0.0..=1.0,
        default: Normalized(0.0),
        flags: ParamFlags::CONST_DEFAULT,
    };
}

pub trait Params: Enum {
    fn metadata(&self) -> ParamMetadata;

    fn value_to_string(&self, value: Normalized) -> String {
        value_to_string_default(value)
    }
    fn string_to_value(&self, string: &str) -> Result<Normalized, String> {
        string_to_value_default(string)
    }
}

pub fn value_to_string_default(value: Normalized) -> String {
    value.to_string()
}

pub fn string_to_value_default(string: &str) -> Result<Normalized, String> {
    let f = string.parse().map_err(|err| format!("{err}"))?;
    Ok(Normalized::new(f).ok_or_else(|| format!("Not normalized: {f}"))?)
}

pub fn enum_range<E: Enum>() -> ops::RangeInclusive<f32> {
    0.0..=(count::<E>() - 1) as f32
}

impl Params for Empty {
    fn metadata(&self) -> ParamMetadata {
        unreachable!()
    }
}

impl<N: Unsigned> Params for Sequential<N>
where
    Self: Enum,
{
    fn metadata(&self) -> ParamMetadata {
        ParamMetadata::CONST_DEFAULT
    }
}
