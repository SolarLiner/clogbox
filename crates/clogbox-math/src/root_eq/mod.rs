//! # Root-finding equations and algorithms
//!
//! This module provides traits and implementations for finding roots of equations,
//! including the Newton-Raphson method for both single-variable and multi-variable equations.

#[cfg(feature = "linalg")]
use nalgebra as na;

pub mod nr;

/// Trait defining an equation and its derivative
///
/// Implemented by types that can evaluate both a function and its derivative
/// at a single point, which is required for single-variable root-finding methods
/// like Newton-Raphson.
pub trait Differentiable {
    /// Scalar type of the equation
    type Scalar: Clone;

    /// Evaluates both the function and its derivative at a point
    ///
    /// # Parameters
    ///
    /// * `x` - The point at which to evaluate the function and its derivative
    ///
    /// # Returns
    ///
    /// A tuple containing (function value, derivative value) at point `x`
    fn eval_with_derivative(&self, x: Self::Scalar) -> (Self::Scalar, Self::Scalar);
}

#[cfg(feature = "linalg")]
/// Trait defining a multivariable root equation and its inverse jacobian
///
/// Implemented by types that can evaluate both a multivariable function and
/// its inverse jacobian at a single point, which is required for multivariable
/// root-finding methods like Newton-Raphson in multiple dimensions.
#[allow(clippy::type_complexity)]
pub trait MultiDifferentiable
where
    na::default_allocator::DefaultAllocator:
        na::allocator::Allocator<Self::Dim> + na::allocator::Allocator<Self::Dim, Self::Dim>,
{
    /// Scalar type of the equations
    type Scalar: na::Scalar;
    /// Dimension of the system
    type Dim: na::DimName;

    /// Evaluates both the function and its inverse jacobian at a point
    ///
    /// # Parameters
    ///
    /// * `x` - The point at which to evaluate the function and its inverse jacobian
    ///
    /// # Returns
    ///
    /// A tuple containing (function value vector, inverse jacobian matrix) at point `x`
    fn eval_with_inv_jacobian(
        &self,
        x: na::VectorView<Self::Scalar, Self::Dim>,
    ) -> (
        na::OVector<Self::Scalar, Self::Dim>,
        na::OMatrix<Self::Scalar, Self::Dim, Self::Dim>,
    );
}
