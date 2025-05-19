//! # `clogbox` Math
//!
//! Mathematical utilities for digital signal processing and audio plugin development.
//!
//! This crate provides various mathematical functions and algorithms commonly used in digital signal processing,
//! including interpolation, root finding, and audio-specific conversions.

#![warn(missing_docs)]

use az::CastFrom;
use num_traits::Float;
use numeric_literals::replace_float_literals;

/// Digital signal processing utilities and algorithms.
///
/// This module contains various DSP-related mathematical functions and algorithms.
pub mod dsp;

/// Interpolation algorithms for signal processing.
///
/// This module provides different interpolation methods used in digital signal processing,
/// such as linear, cubic, and spline interpolation.
pub mod interpolation;

/// Reciprocal approximation algorithms.
///
/// This module contains fast approximation methods for calculating reciprocals (1/x),
/// which can be useful in performance-critical DSP code.
pub mod recip;

/// Root-finding equations and algorithms.
///
/// This module provides traits and implementations for finding roots of equations,
/// including Newton-Raphson method for both single-variable and multi-variable equations.
pub mod root_eq;

/// Converts a decibel value to a linear amplitude value.
///
/// This function converts a value in decibels (dB) to its corresponding linear amplitude.
/// The conversion uses the standard audio formula: linear = 10^(dB/20).
///
/// # Parameters
///
/// * `db` - The decibel value to convert
///
/// # Returns
///
/// The corresponding linear amplitude value
#[replace_float_literals(T::cast_from(literal))]
pub fn db_to_linear<T: Float + CastFrom<f64>>(db: T) -> T {
    10.0_f64.powf(db / 20.0)
}

/// Converts a linear amplitude value to decibels.
///
/// This function converts a linear amplitude to its corresponding value in decibels (dB).
/// The conversion uses the standard audio formula: dB = 20 * log10(linear).
///
/// # Parameters
///
/// * `linear` - The linear amplitude value to convert
///
/// # Returns
///
/// The corresponding decibel value
#[replace_float_literals(T::cast_from(literal))]
pub fn linear_to_db<T: Float + CastFrom<f64>>(linear: T) -> T {
    20.0 * linear.log10()
}
