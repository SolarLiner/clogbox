/// This module provides a comprehensive abstraction for various types of values 
/// that can be used across the application. It offers an enumeration of types 
/// including integers, floats, strings, binary data, and arrays.
///
/// The module also implements conversion traits to and from these types, 
/// enabling seamless transitions between different value representations. 
/// Errors in conversions are handled gracefully with a specific error type 
/// that provides detailed mismatch information.
///
/// # Example
///
/// ```
/// use clogbox_core::param::Value;
///
/// let int_value = Value::from(42);
/// if let Value::Int(i) = int_value {
///     println!("Integer value: {}", i);
/// }
/// ```
pub mod value;
pub mod curve;

use crate::param::value::Value;
use crate::r#enum::Enum;

/// A trait for obtaining parameters.
pub trait GetParameter {
    /// The associated type which must implement the `Enum` trait.
    type Param: Enum;

    /// Gets the raw `Value` for a given parameter.
    ///
    /// # Parameters
    /// 
    /// - `param`: The parameter for which the raw value is to be obtained.
    ///
    /// # Returns
    /// 
    /// - A `Value` corresponding to the provided parameter.
    fn get_param_raw(&self, param: Self::Param) -> Value;

    /// Gets the parameter as a certain type.
    ///
    /// # Parameters
    /// 
    /// - `param`: The parameter for which the value is to be obtained.
    ///
    /// # Returns
    /// 
    /// - A `Result` containing the value converted to type `V`, or an error if the conversion fails.
    fn get_param_as<'a, V: 'a + TryFrom<Value<'a>>>(&'a self, param: Self::Param) -> Result<V, V::Error> {
        self.get_param_raw(param).try_into()
    }
}

/// A trait for setting parameters with various types of values.
pub trait SetParameter: GetParameter {
    /// Sets the raw parameter value.
    ///
    /// # Arguments
    ///
    /// * `param` - The parameter to set.
    /// * `value` - The value to set for the parameter.
    fn set_param_raw(&mut self, param: Self::Param, value: Value);

    /// Sets the parameter value using a type that can be converted into a Value.
    ///
    /// # Arguments
    ///
    /// * `param` - The parameter to set.
    /// * `value` - The value to set for the parameter.
    fn set_param<'a>(&mut self, param: Self::Param, value: impl Into<Value<'a>>) {
        self.set_param_raw(param, value.into())
    }

    /// Attempts to set the parameter value using a type that can be tried into a Value.
    ///
    /// # Arguments
    ///
    /// * `param` - The parameter to set.
    /// * `value` - The value to set for the parameter.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if successful.
    /// * `Err` if conversion of the value failed.
    fn try_set_param<'a, V: TryInto<Value<'a>>>(&mut self, param: Self::Param, value: V) -> Result<(), V::Error> {
        value.try_into().map(|value| self.set_param_raw(param, value))
    }
}

/// Trait for normalizing and unnormalizing parameters.
pub trait NormalizeParameter {
    /// Associated enum type for parameters.
    type Param: Enum;

    /// Normalizes a parameter value into an `Option<f32>`.
    ///
    /// # Parameters
    /// 
    /// - `param`: The parameter to normalize.
    /// - `value`: The value to normalize, convertible into `Value`.
    fn normalize_param<'a>(&self, param: Self::Param, value: impl Into<Value<'a>>) -> Option<f32>;

    /// Unnormalizes a parameter value into an `Option<Value>`.
    ///
    /// # Parameters
    /// 
    /// - `param`: The parameter to unnormalize.
    /// - `value`: The value to unnormalize.
    fn unnormalize_param<'a>(&self, param: Self::Param, value: f32) -> Option<Value<'a>>;
}