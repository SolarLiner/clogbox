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
use num_traits::{Float, FloatConst, ToPrimitive};
use numeric_array::generic_array::IntoArrayLength;
use numeric_array::NumericArray;
use numeric_literals::replace_float_literals;
use std::marker::PhantomData;
use std::ops;
use std::process::Output;
use typenum::{Const, True, Unsigned, U1, U2};

pub enum BoundaryCondition {
    Clamp,
    Wrap,
}

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
    fn interpolate(&self, boundary_condition: BoundaryCondition, values: &impl Collection<Item = T>, index: T) -> T;
}

pub trait InterpolateSingle<T> {
    type Count: IntoArrayLength;

    fn offset_index(&self, index: isize) -> isize {
        index
    }

    fn interpolate_single(
        &self,
        values: &NumericArray<T, <Self::Count as IntoArrayLength>::ArrayLength>,
        index: T,
    ) -> T;
}

impl<T: Float, I: InterpolateSingle<T>> Interpolation<T> for I {
    fn interpolate(&self, boundary_condition: BoundaryCondition, values: &impl Collection<Item = T>, index: T) -> T {
        debug_assert!(values.len() > 0, "Slice to interpolate is empty");
        let f = index.fract();
        let index = index.floor().to_usize().unwrap();
        let indices: NumericArray<_, <I::Count as IntoArrayLength>::ArrayLength> = match boundary_condition {
            BoundaryCondition::Clamp => NumericArray::generate(|i| {
                self.offset_index((index + i) as isize)
                    .to_usize()
                    .unwrap()
                    .clamp(0, values.len() - 1)
            }),
            BoundaryCondition::Wrap => NumericArray::generate(|i| {
                self.offset_index((index + i) as isize)
                    .rem_euclid(values.len() as isize) as usize
            }),
        };
        let array = NumericArray::generate(|i| values.get(indices[i]).copied().unwrap_or(T::zero()));
        self.interpolate_single(&array, f)
    }
}

/// `Linear` is a struct that represents linear interpolation.
///
/// # Examples
/// ```
/// use clogbox_math::interpolation::{BoundaryCondition, Interpolation};
/// use clogbox_math::interpolation::Linear;
///
/// let values = vec![0.0, 1.0, 2.0, 3.0];
/// let index = 1.5;
///
/// // Linear interpolation
/// let interpolated_value = Linear.interpolate(BoundaryCondition::Clamp,&values, index);
///
/// assert_eq!(1.5, interpolated_value);
/// ```
#[derive(Debug, Default, Copy, Clone)]
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
///
/// ```
/// use clogbox_math::interpolation::{BoundaryCondition, Interpolation};
/// use clogbox_math::interpolation::Cubic;
///
/// let values = vec![0f32, 1.0, 4.0, 9.0, 9.0];
/// let index = 1.5;
///
/// // Cubic interpolation
/// assert_eq!(2.25, Cubic.interpolate(BoundaryCondition::Clamp,&values, index));
/// ```
#[derive(Debug, Default, Copy, Clone)]
pub struct Cubic;

impl<T: Float + CastFrom<f64> + Cast<usize>> InterpolateSingle<T> for Cubic {
    type Count = Const<4>;

    fn offset_index(&self, index: isize) -> isize {
        index - 1
    }

    #[replace_float_literals(T::cast_from(literal))]
    fn interpolate_single(&self, p: &NumericArray<T, <Self::Count as IntoArrayLength>::ArrayLength>, x: T) -> T {
        p[1] + x
            * 0.5
            * (p[2] - p[0]
                + x * (2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3] + x * (3.0 * (p[1] - p[2]) + p[3] - p[0])))
    }
}

/// The `Sinc` struct is a generic wrapper designed to work with types implementing the `Unsigned` trait.
/// It is a simple container that holds a value of type `N`.
///
/// # Type Parameters
/// - `N`: A type that must implement the `Unsigned` trait.
///
/// # Derive Attributes
/// - `Debug`: Allows instances of `Sinc` to be formatted using the `{:?}` formatter.
/// - `Copy`: Enables the `Sinc` struct to be copied without moving.
/// - `Clone`: Allows for the explicit creation of a duplicate `Sinc` instance.
/// - `Default`: Provides a default, zero-value initialization for `Sinc`.
///
/// # Example
/// ```
/// use clogbox_math::interpolation::{BoundaryCondition, Interpolation, Sinc};
/// use typenum::U5;
///
/// let sinc_value = Sinc(U5::default());
///
/// let values = vec![0f32, 1.0, 4.0, 9.0, 9.0];
/// let index = 1.5;
///
/// // Cubic interpolation
/// assert_eq!(2.25, sinc_value.interpolate(BoundaryCondition::Clamp, &values, index));
/// ```
#[derive(Debug, Copy, Clone, Default)]
pub struct Sinc<N: Unsigned>(pub N);

impl<T: Float + FloatConst, N: Unsigned + IntoArrayLength + ops::Rem<U2>> InterpolateSingle<T> for Sinc<N>
where
    <N as ops::Rem<U2>>::Output: typenum::type_operators::IsEqual<Output = True>,
{
    type Count = N;

    fn offset_index(&self, index: isize) -> isize {
        index - N::ISIZE / 2
    }

    fn interpolate_single(
        &self,
        values: &NumericArray<T, <Self::Count as IntoArrayLength>::ArrayLength>,
        index: T,
    ) -> T {
        (0..N::USIZE)
            .map(|i| {
                let ki = -index + T::from(i).unwrap() - T::from(N::USIZE / 2).unwrap();
                let k = sinc(T::PI() * ki) * hann::<_, N>(i);
                values[i] * k
            })
            .reduce(ops::Add::add)
            .unwrap_or_else(T::zero)
    }
}

fn hann<T: Float + FloatConst, N: Unsigned>(i: usize) -> T {
    T::sin(T::PI() * T::from(i).unwrap() / T::from(N::USIZE).unwrap()).powi(2)
}

fn sinc<T: Float>(x: T) -> T {
    if x.is_zero() {
        T::one()
    } else {
        x.sin() / x
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

        let result = cubic.interpolate(BoundaryCondition::Clamp, &values, 1.5);
        assert_abs_diff_eq!(result, 2.25);

        let result = cubic.interpolate(BoundaryCondition::Clamp, &values, 0.5);
        assert_abs_diff_eq!(result, 0.3125);

        let result = cubic.interpolate(BoundaryCondition::Clamp, &values, 2.5);
        assert_abs_diff_eq!(result, 6.6875);
    }

    #[test]
    fn test_cubic_interpolate_boundary_conditions() {
        let values: Vec<f64> = vec![0.0, 1.0, 4.0, 9.0];
        let cubic = Cubic;

        // Testing at exact indices to see it returns the same values
        let result = cubic.interpolate(BoundaryCondition::Clamp, &values, 0.0);
        assert_abs_diff_eq!(result, 0.0);

        let result = cubic.interpolate(BoundaryCondition::Clamp, &values, 1.0);
        assert_abs_diff_eq!(result, 1.0);

        let result = cubic.interpolate(BoundaryCondition::Clamp, &values, 2.0);
        assert_abs_diff_eq!(result, 4.0);

        let result = cubic.interpolate(BoundaryCondition::Clamp, &values, 3.0);
        assert_abs_diff_eq!(result, 9.0);
    }

    #[test]
    fn test_cubic_interpolate_out_of_bounds() {
        let values: Vec<f64> = vec![0.0, 1.0, 4.0, 9.0];
        let cubic = Cubic;

        // When index is slightly before the start
        let result = cubic.interpolate(BoundaryCondition::Clamp, &values, -0.5);
        assert_abs_diff_eq!(result, -0.1875);

        // When index is slightly after the end
        let result = cubic.interpolate(BoundaryCondition::Clamp, &values, 3.5);
        assert_abs_diff_eq!(result, 9.3125);
    }
}
