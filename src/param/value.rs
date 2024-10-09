use duplicate::duplicate_item;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Value<'a> {
    Empty,
    Int(i64),
    Float(f32),
    Double(f64),
    String(&'a str),
    Binary(&'a [u8]),
    Array(&'a [Value<'a>]),
}

impl<'a> Value<'a> {
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

#[derive(Debug, Clone, Error)]
pub enum TryFromValueError {
    #[error("Variant mismatch: expected {expected:?}, got {found:?}")]
    VariantMismatch {
        expected: &'static str,
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

impl<'a> Into<Value<'a>> for &'a Path {
    fn into(self) -> Value<'a> {
        // Put here as an `.expect()` instead of a TryInto because it is unlikely (Windows paths are
        // more restrictive, and Unix paths *are* UTF-8 strings)
        Value::String(self.to_str().expect("Path is not a valid UTF-8 string"))
    }
}
