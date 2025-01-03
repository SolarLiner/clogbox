//! This module provides a set of functions for performing basic digital signal processing.
//!
//! ## Example
//!
//! ```rust
//! use clogbox_core::math::dsp::freq_to_z;
//! let z = freq_to_z(44100.0, 1000.0);
//! ```
use num_complex::Complex;
use num_traits::{Float, FloatConst};

/// Converts a frequency to a corresponding point on the complex unit circle (Z-plane).
///
/// # Arguments
///
/// * `sample_rate` - The sampling rate of the signal.
/// * `f` - The frequency to be converted.
///
/// # Returns
///
/// A complex number representing the point on the unit circle corresponding to the given frequency.
///
/// # Examples
///
/// ```
/// use num_complex::Complex;
/// use clogbox_core::math::dsp::freq_to_z;
/// let z = freq_to_z(44100.0, 1000.0);
/// ```
#[inline]
pub fn freq_to_z<T: Float + FloatConst>(sample_rate: T, f: T) -> Complex<T> {
    let jw = Complex::new(T::zero(), T::TAU() * f / sample_rate);
    jw.exp()
}
