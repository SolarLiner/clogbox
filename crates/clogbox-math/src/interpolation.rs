//! This module provides interpolation functionality.
//!
//! It includes definitions and implementations for interpolation over slices.
//!
//! # Usage
//!
//! The `Interpolation` trait is typically used in scenarios where a linear transformation
//! or interpolation is needed, such as in mathematical computations, graphics,
//! or data processing.
use az::{Cast, CastFrom};
use clogbox_enum::enum_map::Collection;
use clogbox_enum::generic_array::sequence::GenericSequence;
use num_traits::Float;
use numeric_array::generic_array::IntoArrayLength;
use numeric_array::NumericArray;
use numeric_literals::replace_float_literals;
use typenum::Const;

/// A trait that defines a method for interpolating values within a [`Collection`] type.
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

/// A trait for single-point interpolation with a fixed number of points.
///
/// This trait defines methods for interpolating a value at a specific index
/// using a fixed number of surrounding points. It is used internally by the
/// `Interpolation` trait to perform the actual interpolation calculation.
///
/// # Type Parameters
///
/// * `T` - The type of the values to interpolate, typically a floating-point type.
pub trait InterpolateSingle<T> {
    /// The number of points used for interpolation.
    ///
    /// This associated type determines how many points are used in the interpolation
    /// calculation. For example, linear interpolation uses 2 points, cubic uses 4.
    type Count: IntoArrayLength;

    /// Adjusts the index before interpolation.
    ///
    /// This method allows implementations to offset the index before interpolation,
    /// which can be useful for certain interpolation algorithms that need to center
    /// the interpolation window differently.
    ///
    /// # Parameters
    ///
    /// * `index` - The original index to adjust
    fn offset_index(&self, index: T) -> T {
        index
    }

    /// Performs the actual interpolation calculation.
    ///
    /// This method implements the specific interpolation algorithm using the provided
    /// values and fractional index.
    ///
    /// # Parameters
    ///
    /// * `values` - An array of values to interpolate between
    /// * `index` - The fractional index within the array (typically between 0 and 1)
    fn interpolate_single(
        &self,
        values: &NumericArray<T, <Self::Count as IntoArrayLength>::ArrayLength>,
        index: T,
    ) -> T;
}

impl<T: Float, I: InterpolateSingle<T>> Interpolation<T> for I {
    fn interpolate(&self, values: &impl Collection<Item = T>, index: T) -> T {
        let min = self.offset_index(index);
        let array = NumericArray::generate(|i| {
            let Some(i) = T::from(i) else {
                return values[min.to_usize().unwrap_or(0)];
            };
            let i = (min + i).clamp(T::zero(), T::from(values.len() - 1).unwrap_or(T::zero()));
            values[i.to_usize().unwrap_or(0)]
        });
        self.interpolate_single(&array, index.fract())
    }
}

/// `Linear` is a struct that represents linear interpolation.
///
/// # Examples
/// ```
/// use clogbox_math::interpolation::Interpolation;
/// use clogbox_math::interpolation::Linear;
///
/// let values = vec![0.0, 1.0, 2.0, 3.0];
/// let index = 1.5;
///
/// // Linear interpolation
/// let interpolated_value = Linear.interpolate(&values, index);
///
/// assert_eq!(1.5, interpolated_value);
/// ```
#[derive(Debug, Copy, Clone)]
pub struct Linear;

impl<T: Copy + Float + Cast<usize>> InterpolateSingle<T> for Linear {
    type Count = Const<2>;

    fn interpolate_single(
        &self,
        values: &NumericArray<T, <Self::Count as IntoArrayLength>::ArrayLength>,
        index: T,
    ) -> T {
        values[0] + (values[1] - values[0]) * index
    }
}

/// `Cubic` is a struct that represents cubic interpolation.
///
/// # Examples
/// ```
/// use clogbox_math::interpolation::Interpolation;
/// use clogbox_math::interpolation::Cubic;
///
/// let values = vec![0f32, 1.0, 4.0, 9.0, 9.0];
/// let index = 1.5;
///
/// // Cubic interpolation
/// assert_eq!(2.25, Cubic.interpolate(&values, index));
/// ```
pub struct Cubic;

impl<T: Float + CastFrom<f64> + Cast<usize>> InterpolateSingle<T> for Cubic {
    type Count = Const<4>;

    fn offset_index(&self, index: T) -> T {
        index - T::cast_from(1.0)
    }

    #[replace_float_literals(T::cast_from(literal))]
    fn interpolate_single(&self, p: &NumericArray<T, <Self::Count as IntoArrayLength>::ArrayLength>, x: T) -> T {
        p[1] + x
            * 0.5
            * (p[2] - p[0]
                + x * (2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3] + x * (3.0 * (p[1] - p[2]) + p[3] - p[0])))
    }
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

        // When the index is slightly before the start
        let result = cubic.interpolate(&values, -0.5);
        assert_abs_diff_eq!(result, -0.1875);

        // When the index is slightly after the end
        let result = cubic.interpolate(&values, 3.5);
        assert_abs_diff_eq!(result, 9.3125);
    }
}
