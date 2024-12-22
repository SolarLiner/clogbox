//! This module provides interpolation functionality.
//!
//! It includes definitions and implementations for interpolation over slices.
//!
//! # Usage
//!
//! The `Interpolation` trait is typically used in scenarios where a linear transformation
//! or interpolation is needed, such as in mathematical computations, graphics,
//! or data processing.
use clogbox_enum::enum_map::Collection;
use clogbox_enum::Count;
use az::{Cast, CastFrom};
use num_traits::{Float, Num};
use numeric_array::generic_array::IntoArrayLength;
use numeric_array::NumericArray;
use numeric_literals::replace_float_literals;
use typenum::{Const, Unsigned};

/// A trait that defines a method for interpolating values within a [`Collection`](clogbox_enum::enum_map::Collection) type.
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

pub trait InterpolateSingle<T> {
    type Count: IntoArrayLength;

    fn offset_index(&self, index: T) -> T {
        index
    }

    fn interpolate_single(
        &self,
        values: &NumericArray<T, <Self::Count as IntoArrayLength>::ArrayLength>,
        index: T,
    ) -> T;
}

impl<T: Float, I: InterpolateSingle<T>> Interpolation<T> for I {
    fn interpolate(&self, values: &impl Collection<Item = T>, index: T) -> T {
        let min = self.offset_index(index).floor().to_usize().unwrap();
        let max = values
            .len()
            .min(min + <<I as InterpolateSingle<T>>::Count as IntoArrayLength>::ArrayLength::USIZE);
        let array = NumericArray::from_slice(&values[min..max]);
        self.interpolate_single(array, index.fract())
    }
}

/// `Linear` is a struct that represents linear interpolation.
///
/// # Examples
/// ```
/// use clogbox_core::math::interpolation::Interpolation;
/// use clogbox_core::math::interpolation::Linear;
///
/// let values = vec![0.0, 1.0, 2.0, 3.0];
/// let index = 1.5;
///
/// // Linear interpolation
/// let interpolated_value = Linear.interpolate(&values, index);
///
/// assert_eq!(1.5, interpolated_value);
/// ```
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

impl<T: Float + CastFrom<f64> + Cast<usize>> InterpolateSingle<T> for Cubic {
    type Count = Const<4>;

    fn offset_index(&self, index: T) -> T {
        index - T::cast_from(1.0)
    }

    #[replace_float_literals(T::cast_from(literal))]
    fn interpolate_single(
        &self,
        p: &NumericArray<T, <Self::Count as IntoArrayLength>::ArrayLength>,
        x: T,
    ) -> T {
        p[1] + x
            * 0.5
            * (p[2] - p[0]
                + x * (2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3]
                    + x * (3.0 * (p[1] - p[2]) + p[3] - p[0])))
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

        // When index is slightly before the start
        let result = cubic.interpolate(&values, -0.5);
        assert_abs_diff_eq!(result, 0.0);

        // When index is slightly after the end
        let result = cubic.interpolate(&values, 3.5);
        assert_abs_diff_eq!(result, 9.0);
    }
}
