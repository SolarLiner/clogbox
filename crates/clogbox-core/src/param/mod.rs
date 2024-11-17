use crate::math::recip::Recip;
use az::{Cast, CastFrom};
use num_complex::ComplexFloat;
use num_traits::{AsPrimitive, Float, NumAssign};
use std::any::TypeId;
use std::fmt::{Formatter, Write};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::{fmt, ops};
use std::borrow::Cow;
use std::num::NonZeroUsize;
use std::ops::RangeInclusive;
use crate::r#enum::enum_map::{Collection, EnumMap, EnumMapArray};
use crate::r#enum::{count, Empty, Enum};

#[cfg(feature = "data-param")]
pub use bincode::de;
#[cfg(feature = "data-param")]
pub use bincode::enc;
use bitflags::bitflags;

pub mod smoother;

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub enum FloatMapping {
    #[default]
    Linear,
    Logarithmic,
    Exponential,
    Sqrt,
}

#[derive(Debug, Clone)]
pub struct FloatRange {
    pub range: ops::RangeInclusive<f32>,
    pub mapping: FloatMapping,
}

impl Default for FloatRange {
    fn default() -> Self {
        Self::CONST_DEFAULT
    }
}

fn lerp(x: f32, a: f32, b: f32) -> f32 {
    a + x * (b - a)
}

fn unlerp(x: f32, a: f32, b: f32) -> f32 {
    (x - a) / (b - a)
}

impl FloatRange {
    pub const CONST_DEFAULT: Self = Self {
        range: 0.0..=1.0,
        mapping: FloatMapping::Linear,
    };

    pub const fn new(range: ops::RangeInclusive<f32>) -> Self {
        Self {
            range,
            mapping: FloatMapping::Linear,
        }
    }

    pub const fn with_range(mut self, range: ops::RangeInclusive<f32>) -> Self {
        self.range = range;
        self
    }

    pub const fn with_mapping(mut self, mapping: FloatMapping) -> Self {
        self.mapping = mapping;
        self
    }

    pub fn map(&self, value: f32) -> f32 {
        match self.mapping {
            FloatMapping::Linear => unlerp(value, *self.range.start(), *self.range.end()),
            FloatMapping::Logarithmic => unlerp(
                value.log2(),
                self.range.start().log2(),
                self.range.end().log2(),
            )
            .exp2(),
            FloatMapping::Exponential => unlerp(
                value.exp2(),
                self.range.start().exp2(),
                self.range.end().exp2(),
            )
            .log2(),
            FloatMapping::Sqrt => unlerp(
                value.sqrt(),
                self.range.start().sqrt(),
                self.range.end().sqrt(),
            )
            .powi(2),
        }
    }

    pub fn unmap(&self, normalized_value: f32) -> f32 {
        match self.mapping {
            FloatMapping::Linear => lerp(normalized_value, *self.range.start(), *self.range.end()),
            FloatMapping::Logarithmic => lerp(normalized_value, self.range.start().exp2(), self.range.end().exp2()).log2(),
            FloatMapping::Exponential => lerp(normalized_value, self.range.start().log2(), self.range.end().log2()).exp2(),
            FloatMapping::Sqrt => lerp(normalized_value, self.range.start().sqrt(), self.range.end().sqrt()).powi(2),
        }
    }
}

#[derive(Debug)]
pub struct Value {
    value: AtomicU32,
    range: FloatRange,
    has_changed: AtomicBool,
}

impl Value {
    pub const fn new(value: f32) -> Self {
        Self {
            // Safety: This is safe as both f32 and u32 have same size and alignment
            // TODO: Switch to .to_bits() when it is const stable
            value: AtomicU32::new(unsafe { *(&value as *const f32 as *const u32) }),
            range: FloatRange::CONST_DEFAULT,
            has_changed: AtomicBool::new(false),
        }
    }

    pub const fn for_enum<E: Enum>(value: E) -> Self {
        Self::new(value.cast() as _).with_range(FloatRange::new(0.0..=count::<E>() as f32))
    }

    pub const fn with_range(mut self, range: FloatRange) -> Self {
        self.range = range;
        self
    }

    pub fn get_value(&self) -> f32 {
        f32::from_bits(self.value.load(Ordering::Relaxed))
    }

    pub fn get_int(&self) -> i32 {
        self.get_value().round() as _
    }

    pub fn get_bool(&self) -> bool {
        self.get_value() > 0.5
    }

    pub fn get_enum<E: Enum>(&self) -> E {
        let i: usize = self.range.range.end().into();
        assert_eq!(count::<E>(), i);
        E::cast_from(self.get_value().floor() as _)
    }

    pub fn set_value(&self, value: f32) {
        self.value.store(value.to_bits(), Ordering::Relaxed);
        self.has_changed.store(true, Ordering::Relaxed);
    }

    pub fn set_int(&self, value: i32) {
        self.set_value(value as _)
    }

    pub fn set_bool(&self, value: bool) {
        self.set_value(if value { 1.0 } else { 0.0 })
    }

    pub fn set_enum<E: Enum>(&self, value: E) {
        let c: usize = self.range.range.end().into();
        assert_eq!(count::<E>(), c);
        self.set_value(value.cast() as _)
    }

    pub fn normalize(&self, value: f32) -> f32 {
        self.range.map(value)
    }

    pub fn demornamlize(&self, normalized: f32) -> f32 {
        self.range.unmap(normalized)
    }

    pub fn get_normalized(&self) -> f32 {
        self.normalize(self.get_value())
    }

    pub fn set_normalized(&self, normalized: f32) {
        self.set_value(self.demornamlize(normalized))
    }

    pub fn has_changed(&self) -> bool {
        self.has_changed.swap(false, Ordering::Relaxed)
    }

    pub fn touch(&self) {
        self.has_changed.store(true, Ordering::Relaxed);
    }
}

#[cfg(feature = "data-param")]
#[derive(enc::Encode, de::Decode, serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct DataValue {
    data: Vec<u8>,
}

#[cfg(feature = "data-param")]
impl fmt::Debug for DataValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            let sizesize = self.data.len().checked_ilog10().map(|n| n + 1).unwrap_or(1) as usize;
            for _ in 0..=sizesize {
                f.write_char(' ')?
            }
            writeln!(f, "| 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F |")?;
            for _ in 0..=sizesize {
                f.write_char(' ')?
            }
            writeln!(f, "|-------------------------------------------------|")?;
            for (i, chunk) in self.data.chunks(16).enumerate() {
                write!(f, "{:0sizesize$x} |", i * 16, sizesize = sizesize)?;
                for byte in chunk {
                    write!(f, " {:02x}", byte)?;
                }
                for _ in chunk.len()..16 {
                    f.write_str("   ")?;
                }
                f.write_str(" |")?;
                for byte in chunk {
                    if *byte >= 0x20 && *byte <= 0x7e {
                        write!(f, "{}", *byte as char)?;
                    } else {
                        f.write_char('.')?;
                    }
                }
                f.write_char('\n')?;
            }
            Ok(())
        } else {
            f.debug_struct("DataParameter")
                .field(
                    "data",
                    &format!("<binary data of {} bytes>", self.data.len()),
                )
                .finish()
        }
    }
}

#[cfg(feature = "data-param")]
impl DataValue {
    const CONFIG: bincode::config::Configuration = bincode::config::standard()
        .with_no_limit()
        .with_little_endian()
        .with_variable_int_encoding();

    /// Creates a new `DataParameter` instance with the provided value.
    ///
    /// This method allocates to store the serialized data.
    pub fn new<T: enc::Encode>(value: T) -> Result<Self, bincode::error::EncodeError> {
        let data = bincode::encode_to_vec(value, Self::CONFIG)?;
        Ok(Self { data })
    }

    /// Creates a new `DataParameter` instance with the provided binary data.
    pub fn from_binary(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Decodes a `T` from the binary data.
    pub fn get_value<'a, T: de::BorrowDecode<'a>>(
        &'a self,
    ) -> Result<T, bincode::error::DecodeError> {
        let (value, _) = bincode::borrow_decode_from_slice(&self.data, Self::CONFIG)?;
        Ok(value)
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
    #[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
    pub struct ParamFlags: u16 {
        const INTERNAL = 0 << 0;
        const MODULABLE = 0 << 1;
        const AUTOMATABLE = 0 << 2;
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct Parameter {
    value: Value,
    flags: ParamFlags,
    default_value: f32,
    name: Cow<'static, str>,
}

impl Parameter {
    pub const fn new(name: Cow<'static, str>, default_value: f32, value: Value) -> Self {
        Self {
            value,
            flags: ParamFlags::empty(),
            default_value,
            name,
        }
    }

    pub const fn with_flags(mut self, flags: ParamFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn flags(&self) -> ParamFlags {
        self.flags
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}

pub trait Params {
    type Params: Enum;

    fn module_name(&self) -> Option<Cow<'static, str>> {
        None
    }

    fn get_param(&self, param: Self::Params) -> &Parameter;
}

impl<E: Enum, C: Collection<Item=Parameter>> Params for EnumMap<E, C> {
    type Params = E;

    fn get_param(&self, param: Self::Params) -> &Parameter {
        &self[param]
    }
}

pub trait RawParams {
    fn module_name(&self) -> Option<Cow<'static, str>> {
        None
    }
    fn num_params(&self) -> usize;
    fn get_param(&self, index: usize) -> Option<&Parameter>;
}

impl<P: Params> RawParams for P {
    fn module_name(&self) -> Option<Cow<'static, str>> {
        P::module_name(self)
    }
    fn num_params(&self) -> usize {
        count::<P::Params>()
    }
    fn get_param(&self, index: usize) -> Option<&Parameter> {
        (index < count::<P::Params>()).then(|| P::get_param(self, P::Params::cast_from(index)))
    }
}

pub const EMPTY_PARAMS: EnumMapArray<Empty, Parameter> = EnumMapArray::CONST_DEFAULT;

#[cfg(test)]
mod tests {
    #[cfg(feature = "data-param")]
    use crate::param::DataValue;
    use crate::param::smoother::{ExponentialSmoother, LinearSmoother};
    use crate::param::{Parameter, Value};

    #[test]
    fn test_param_send_sync() {
        fn ensure_send_sync<T: Send + Sync>() {}

        ensure_send_sync::<Value>();
        #[cfg(feature = "data-param")]
        ensure_send_sync::<DataValue>();
    }

    #[test]
    fn test_float_parameter() {
        let param = Value::new(0.0);
        assert_eq!(0.0, param.get_value());
        assert!(!param.has_changed());
        param.set_value(1.0);
        assert_eq!(1.0, param.get_value());
        assert!(param.has_changed());
    }

    #[test]
    #[cfg(feature = "data-param")]
    fn test_data_parameter() {
        #[derive(bincode::Encode, bincode::Decode, PartialEq, Debug)]
        struct Data {
            a: u32,
            b: f32,
        }

        let expected = Data { a: 0, b: 1.5 };
        let param = DataValue::new(&expected).unwrap();
        insta::assert_debug_snapshot!(param);
        assert_eq!(expected, param.get_value().unwrap());
    }
}
