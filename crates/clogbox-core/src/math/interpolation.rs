//! This module provides interpolation functionality.
//!
//! It includes definitions and implementations for interpolation over slices.
//!
//! # Usage
//!
//! The `Interpolation` trait is typically used in scenarios where a linear transformation
//! or interpolation is needed, such as in mathematical computations, graphics,
//! or data processing.
use crate::r#enum::enum_map::Collection;
use az::{Cast, CastFrom};
use num_traits::{Float, Num};
use numeric_literals::replace_float_literals;

/// A trait that defines a method for interpolating values within a [`Collection`](crate::r#enum::enum_map::Collection) type.
///
/// # Type Parameters
///
/// * `T` - The type of the items to interpolate.
///
pub trait Interpolation<T> {
    /// Interpolates a value at a specified index within a collection of values.
    ///
    /// # Arguments
    ///
    /// * `values` - A collection of items to interpolate from.
    /// * `index` - The index at which to interpolate the value.
    ///
    /// # Returns
    ///
    /// The interpolated value at the specified index.
    fn interpolate(&self, values: &impl Collection<Item = T>, index: T) -> T;
}

/// `Linear` is a struct that represents linear interpolation.
///
/// # Usage
///
/// The `Linear` struct is typically used in scenarios where a linear transformation
/// is needed, such as in mathematical computations, graphics, or data processing.
///
/// This struct does not currently contain any fields or methods, and its primary
/// purpose is to serve as a type marker or placeholder in more complex systems.
///
/// # Examples
/// ```
/// use clogbox_core::math::interpolation::Interpolation;
/// use clogbox_core::math::interpolation::Linear;
///
/// // Example of creating a Linear instance
/// ///
/// // Assume we have a collection of values to interpolate
/// let values = vec![0.0, 1.0, 2.0, 3.0];
/// let index = 1.5;
///
/// // Linear interpolation
/// let interpolated_value = Linear.interpolate(&values, index);
///
/// assert_eq!(1.5, interpolated_value);
/// ```
pub struct Linear;

impl<T: Copy + Float + Cast<usize>> Interpolation<T> for Linear {
    fn interpolate(&self, values: &impl Collection<Item = T>, index: T) -> T {
        let i = index.floor().cast();
        let j = T::cast(index + T::one());
        let a = values[i];
        let b = values[j];
        a + (b - a) * index.fract()
    }
}
/// `Cubic` is a struct that represents cubic interpolation.
///
/// # Usage
///
/// The `Cubic` struct is typically used in scenarios where a cubic transformation
/// or interpolation is needed, such as in mathematical computations, graphics,
/// or data processing.
///
/// This struct does not currently contain any fields or methods, and its primary
/// purpose is to serve as a type marker or placeholder in more complex systems.
///
/// # Examples
/// ```
/// use clogbox_core::math::interpolation::Interpolation;
/// use clogbox_core::math::interpolation::Cubic;
///
/// let values = vec![0.0, 1.0, 4.0, 9.0];
/// let index = 1.5;
///
/// // Cubic interpolation
/// assert_eq!(2.25, Cubic.interpolate(&values, index));
/// ```
pub struct Cubic;

impl<T: Float + CastFrom<f64> + Cast<usize>> Interpolation<T> for Cubic {
    #[replace_float_literals(T::cast_from(literal))]
    fn interpolate(&self, values: &impl Collection<Item = T>, index: T) -> T {
        debug_assert!(!values.is_empty());
        if index < 0.0 {
            return values[0];
        }
        if index > T::cast_from((values.len() - 1) as f64) {
            return values[values.len() - 1];
        }
        let i = index.floor().cast();
        let ip1 = (i + 1).min(values.len() - 1);
        let im1 = i.saturating_sub(1);
        let ip2 = (i + 2).min(values.len() - 1);

        let p0 = values[im1];
        let p1 = values[i];
        let p2 = values[ip1];
        let p3 = values[ip2];

        cubic_interpolate([p0, p1, p2, p3], index.fract())
    }
}

#[replace_float_literals(T::cast_from(literal))]
fn cubic_interpolate<T: Copy + CastFrom<f64> + Num>(p: [T; 4], x: T) -> T {
    p[1] + x
        * 0.5
        * (p[2] - p[0]
            + x * (2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3]
                + x * (3.0 * (p[1] - p[2]) + p[3] - p[0])))
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn test_cubic_interpolate() {
        let values: Vec<f64> = vec![0.0, 1.0, 4.0, 9.0];
        let cubic = Cubic;

        let result = cubic.interpolate(&values, 1.5);
        assert_abs_diff_eq!(result, 2.25);

        let result = cubic.interpolate(&values, 0.5);
        assert_abs_diff_eq!(result, 0.3125);

        let result = cubic.interpolate(&values, 2.5);
        assert_abs_diff_eq!(result, 6.6875);
    }

    #[test]
    fn test_cubic_interpolate_boundary_conditions() {
        let values: Vec<f64> = vec![0.0, 1.0, 4.0, 9.0];
        let cubic = Cubic;

        // Testing at exact indices to see it returns the same values
        let result = cubic.interpolate(&values, 0.0);
        assert_abs_diff_eq!(result, 0.0);

        let result = cubic.interpolate(&values, 1.0);
        assert_abs_diff_eq!(result, 1.0);

        let result = cubic.interpolate(&values, 2.0);
        assert_abs_diff_eq!(result, 4.0);

        let result = cubic.interpolate(&values, 3.0);
        assert_abs_diff_eq!(result, 9.0);
    }

    #[test]
    fn test_cubic_interpolate_out_of_bounds() {
        let values: Vec<f64> = vec![0.0, 1.0, 4.0, 9.0];
        let cubic = Cubic;

        // When index is slightly before the start
        let result = cubic.interpolate(&values, -0.5);
        assert_abs_diff_eq!(result, 0.0);

        // When index is slightly after the end
        let result = cubic.interpolate(&values, 3.5);
        assert_abs_diff_eq!(result, 9.0);
    }
}
