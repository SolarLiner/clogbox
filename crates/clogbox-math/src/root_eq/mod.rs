pub mod nr;

/// Trait defining an equation and its derivative
pub trait Differentiable {
    /// Scalar type of the equation
    type Scalar: Clone;

    /// Evaluates both the function and its derivative at a point
    fn eval_with_derivative(&self, x: Self::Scalar) -> (Self::Scalar, Self::Scalar);
}
