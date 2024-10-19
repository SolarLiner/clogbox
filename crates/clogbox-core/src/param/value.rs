//! Module for handling various types of values in a hierarchical manner.
//!
//! This module provides an enumeration to represent different types of data values,
//! including integers, floating-point numbers, strings, binary data, and arrays of values.
//! It is inspired by the `serde_json::Value` enumeration, but with a focus on zero-copy
//! operations suitable for real-time contexts. The module includes functionality for 
//! conversion between these value types and common Rust types, with error handling for
//! mismatched types.
//!
//! Example:
//! ```
//! use clogbox_core::param::Value;
//!
//! let int_value: Value = 42.into();
//! let float_value: Value = 3.14.into();
//! let str_value: Value = "hello".into();
//!
//! assert_eq!(int_value.variant_str(), "int");
//! assert_eq!(float_value.variant_str(), "float");
//! assert_eq!(str_value.variant_str(), "string");
//! ```
use duplicate::duplicate_item;
use std::path::Path;
use thiserror::Error;

/// Represents various types of values.
///
/// This takes inspiration from `serde_json::Value`, but is zero-copy, allowing its use in real-time
/// contexts.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Value<'a> {
    /// An empty value.
    Empty,
    /// An integer value.
    Int(i64),
    /// A single-precision floating point value.
    Float(f32),
    /// A double-precision floating point value.
    Double(f64),
    /// A string slice value.
    String(&'a str),
    /// A binary value.
    Binary(&'a [u8]),
    /// An array of values.
    Array(&'a [Value<'a>]),
}

impl<'a> Value<'a> {
    /// Returns a string representation of the variant.
    pub fn variant_str(&self) -> &'static str {
        match self {
            Value::Empty => "empty",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Double(_) => "double",
            Value::String(_) => "string",
            Value::Binary(_) => "binary",
            Value::Array(_) => "array",
        }
    }
}

#[duplicate_item(
ty          variant;
[()]        [Empty];
[i64]       [Int(value)];
[f32]       [Float(value)];
[f64]       [Double(value)];
)]
#[allow(unused_variables)]
impl From<ty> for Value<'static> {
    fn from(value: ty) -> Self {
        Self::variant
    }
}

#[duplicate_item(
ty                  variant;
[&'a str]           [String(value)];
[&'a [u8]]          [Binary(value)];
[&'a [Value<'a>]]   [Array(value)];
)]
impl<'a> From<ty> for Value<'a> {
    fn from(value: ty) -> Self {
        Self::variant
    }
}

/// Error type for failed conversions from a value.
#[derive(Debug, Clone, Error)]
pub enum TryFromValueError {
    /// Error indicating a variant mismatch during a conversion.
    #[error("Variant mismatch: expected {expected:?}, got {found:?}")]
    VariantMismatch {
        /// The expected variant name.
        expected: &'static str,
        /// The found variant name.
        found: &'static str,
    },
}

#[duplicate_item(
ty                  variant         result         expected_variant;
[()]                [Empty]         [()]           ["empty"];
[i64]               [Int(value)]    [value]        ["int"];
[f32]               [Float(value)]  [value]        ["float"];
[f64]               [Double(value)] [value]        ["float"];
[&'a str]           [String(value)] [value]        ["string"];
[&'a [u8]]          [Binary(value)] [value]        ["binary"];
[&'a [Value<'a>]]   [Array(value)]  [value]        ["array"];
)]
impl<'a> TryFrom<Value<'a>> for ty {
    type Error = TryFromValueError;

    fn try_from(value: Value<'a>) -> Result<Self, Self::Error> {
        if let Value::variant = value {
            Ok(result)
        } else {
            Err(TryFromValueError::VariantMismatch {
                expected: expected_variant,
                found: value.variant_str(),
            })
        }
    }
}

impl<'a> TryFrom<Value<'a>> for &'a Path {
    type Error = TryFromValueError;

    fn try_from(value: Value<'a>) -> Result<Self, Self::Error> {
        match value {
            Value::String(str) => Ok(Path::new(str)),
            _ => Err(TryFromValueError::VariantMismatch {
                expected: "string",
                found: value.variant_str(),
            }),
        }
    }
}

impl<'a> From<&'a Path> for Value<'a> {
    fn from(val: &'a Path) -> Self {
        // Put here as an `.expect()` instead of a TryInto because it is unlikely (Windows paths are
        // more restrictive, and Unix paths *are* UTF-8 strings)
        Value::String(val.to_str().expect("Path is not a valid UTF-8 string"))
    }
}