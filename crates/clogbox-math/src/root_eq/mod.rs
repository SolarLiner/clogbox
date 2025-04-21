pub mod nr;

/// Trait defining an equation and its derivative
pub trait Differentiable {
    /// Scalar type of the equation
    type Scalar: Clone;

    /// Evaluates the function at a point
    fn eval(&self, x: Self::Scalar) -> Self::Scalar;

    /// Evaluates the derivative of the function at a point
    fn derivative(&self, x: Self::Scalar) -> Self::Scalar;

    /// Evaluates both the function and its derivative at a point
    fn eval_with_derivative(&self, x: Self::Scalar) -> (Self::Scalar, Self::Scalar) {
        (self.eval(x.clone()), self.derivative(x))
    }
}
