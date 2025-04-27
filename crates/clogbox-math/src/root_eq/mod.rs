#[cfg(feature = "linalg")]
use nalgebra as na;

pub mod nr;

/// Trait defining an equation and its derivative
pub trait Differentiable {
    /// Scalar type of the equation
    type Scalar: Clone;

    /// Evaluates both the function and its derivative at a point
    fn eval_with_derivative(&self, x: Self::Scalar) -> (Self::Scalar, Self::Scalar);
}

#[cfg(feature = "linalg")]
/// Trait defining a root equation and its inverse jacobian
pub trait MultiDifferentiable
where
    na::default_allocator::DefaultAllocator:
        na::allocator::Allocator<Self::Dim> + na::allocator::Allocator<Self::Dim, Self::Dim>,
{
    /// Scalar type of the equations
    type Scalar: na::Scalar;
    /// Dimension of the system
    type Dim: na::DimName;

    fn eval_with_inv_jacobian(
        &self,
        x: na::VectorView<Self::Scalar, Self::Dim>,
    ) -> (
        na::OVector<Self::Scalar, Self::Dim>,
        na::OMatrix<Self::Scalar, Self::Dim, Self::Dim>,
    );
}
