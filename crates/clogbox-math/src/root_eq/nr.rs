//! Newton-Raphson root-finding methods

use crate::root_eq::Differentiable;
#[cfg(feature = "linalg")]
use crate::root_eq::MultiDifferentiable;
#[cfg(feature = "linalg")]
use nalgebra as na;
#[cfg(feature = "linalg")]
use nalgebra::RealField;
use num_traits::Float;
#[cfg(feature = "linalg")]
use num_traits::{NumAssign, Zero};
use std::ops;

/// Newton-Raphson solver
pub struct NewtonRaphson<T> {
    /// Maximum number of iterations to perform. Exceeding this can be checked with [`SolveResult`], where
    /// $ \text{delta} > 0 $ and [`SolveResult::iterations`] $ == \text{iterations} $.
    pub max_iterations: usize,
    /// Tolerance for errors in solving the root equation. For scalar equations (implementing [`Differentiable`]),
    /// this will be the maximum delta of an iteration that will stop computation. For vector equations
    /// (implementing [`MultiDifferentiable`]), this will be the maximum [RMS] error that will stop computation.
    ///
    /// [RMS]: https://en.wikipedia.org/wiki/Root_mean_square
    pub tolerance: T,
}

impl<T> NewtonRaphson<T> {
    /// Create a new [`NewtonRhapson`](Self) instance with the provided settings.
    pub const fn new(max_iterations: usize, tolerance: T) -> Self
    where
        T: ops::Add<Output = T>,
    {
        Self {
            max_iterations,
            tolerance,
        }
    }
}

/// Type returned by [`NewtonRhapson`](Self) at the end of computation. Contains the computed value, but also the
/// delta at
/// the time and the number of iterations used.
#[derive(Debug, Clone, Copy)]
pub struct SolveResult<T> {
    /// Result value.
    pub value: T,
    /// Error delta at the value.
    pub delta: T,
    /// Number of iterations used to compute the value.
    pub iterations: usize,
}

impl<T> NewtonRaphson<T> {
    /// Solves the equation using the Newton-Raphson method
    ///
    /// # Arguments
    ///
    /// * `root_equation`: Equation to find the root for.
    /// * `initial_guess`: Initial guess. A good initial guess will vastly improve the performance of the solver, so
    /// it is wise to spend time to tweak it and try to optimize for it. A good heuristic for this value will be both
    /// fast to compute and also make the solver faster as a result.
    pub fn solve<F: Differentiable<Scalar = T>>(&self, root_equation: &F, initial_guess: T) -> SolveResult<T>
    where
        T: Float,
    {
        let mut x = initial_guess;

        for i in 0..self.max_iterations {
            let (fx, dfx) = root_equation.eval_with_derivative(x);
            let delta = fx / dfx;

            x = x - delta;

            if delta.abs() < self.tolerance {
                return SolveResult {
                    value: x,
                    delta,
                    iterations: i,
                };
            }
        }

        SolveResult {
            value: x,
            delta: T::zero(),
            iterations: self.max_iterations,
        }
    }
}

#[cfg(feature = "linalg")]
impl<T: Copy + na::Scalar + NumAssign + RealField + PartialOrd + Zero> NewtonRaphson<T> {
    /// Solves the equation using the Newton-Raphson method
    ///
    /// # Arguments
    ///
    /// * `root_equation`: Equation to find the root for.
    /// * `value`: Parameter containing the initial guess. A good initial guess will vastly improve the performance of
    /// the solver, so it is wise to spend time to tweak it and try to optimize for it. A good heuristic for this
    /// value will be both fast to compute and also make the solver faster as a result. This value is mutated in
    /// place and will contain the final value after the solver is done.
    pub fn solve_multi<F: MultiDifferentiable<Scalar = T>>(
        &self,
        root_equation: &F,
        mut value: na::VectorViewMut<T, F::Dim>,
    ) -> SolveResult<na::OVector<T, F::Dim>>
    where
        na::default_allocator::DefaultAllocator:
            na::allocator::Allocator<F::Dim> + na::allocator::Allocator<F::Dim, F::Dim>,
    {
        for i in 0..self.max_iterations {
            let (fx, inv_j) = root_equation.eval_with_inv_jacobian(value.as_view());
            let delta = inv_j * fx;

            if delta.iter().any(|x| !x.is_finite()) {
                return SolveResult {
                    value: value.into_owned(),
                    delta: na::OVector::repeat(T::zero() / T::zero()),
                    iterations: i,
                };
            }

            value -= &delta;

            let rms = delta.magnitude();
            if rms < self.tolerance {
                return SolveResult {
                    value: value.clone_owned(),
                    delta,
                    iterations: i,
                };
            }
        }

        SolveResult {
            value: value.into_owned(),
            delta: na::zero(),
            iterations: self.max_iterations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    // Simple quadratic function: f(x) = x² - 4 (roots at x = ±2)
    struct Quadratic;
    impl Differentiable for Quadratic {
        type Scalar = f64;

        fn eval_with_derivative(&self, x: Self::Scalar) -> (Self::Scalar, Self::Scalar) {
            (x * x - 4.0, 2.0 * x)
        }
    }

    // Cubic function: f(x) = x³ - 2x² - 11x + 12 (roots at x = -3, 1, 4)
    struct Cubic;
    impl Differentiable for Cubic {
        type Scalar = f64;

        fn eval_with_derivative(&self, x: Self::Scalar) -> (Self::Scalar, Self::Scalar) {
            (
                x.powi(3) - 2.0 * x.powi(2) - 11.0 * x + 12.0,
                3.0 * x.powi(2) - 4.0 * x - 11.0,
            )
        }
    }

    // Trigonometric function: f(x) = sin(x) (roots at x = 0, ±π, ±2π, etc.)
    struct Sine;
    impl Differentiable for Sine {
        type Scalar = f64;

        fn eval_with_derivative(&self, x: Self::Scalar) -> (Self::Scalar, Self::Scalar) {
            x.sin_cos()
        }
    }

    struct Atanh {
        x: f64,
    }
    impl Differentiable for Atanh {
        type Scalar = f64;

        fn eval_with_derivative(&self, y: Self::Scalar) -> (Self::Scalar, Self::Scalar) {
            (y.tanh() - self.x, (1.0 - y.powi(2)).recip())
        }
    }

    #[test]
    fn test_newton_raphson_quadratic() {
        let nr = NewtonRaphson::new(100, 1e-10);

        // Starting from positive values should find the positive root
        let result = nr.solve(&Quadratic, 3.0);
        assert!(result.iterations < nr.max_iterations);
        assert!((result.value - 2.0).abs() < nr.tolerance);

        // Starting from negative values should find the negative root
        let result = nr.solve(&Quadratic, -3.0);
        assert!(result.iterations < nr.max_iterations);
        assert!((result.value + 2.0).abs() < nr.tolerance);
    }

    #[test]
    fn test_newton_raphson_cubic() {
        let nr = NewtonRaphson::new(100, 1e-10);

        // Test finding each of the three roots based on initial guess
        let result = nr.solve(&Cubic, -4.0);
        assert!(result.iterations < nr.max_iterations);
        assert!((result.value + 3.0).abs() < nr.tolerance);

        let result = nr.solve(&Cubic, 0.5);
        assert!(result.iterations < nr.max_iterations);
        assert!((result.value - 1.0).abs() < nr.tolerance);

        let result = nr.solve(&Cubic, 3.0);
        assert!(result.iterations < nr.max_iterations);
        assert!((result.value - 4.0).abs() < nr.tolerance);
    }

    #[test]
    fn test_newton_raphson_sine() {
        let nr = NewtonRaphson::new(100, 1e-10);

        // Find the root at x = 0
        let result = nr.solve(&Sine, 0.1);
        assert!(result.iterations < nr.max_iterations);
        assert!(result.value.abs() < nr.tolerance);

        // Find the root at x = π
        let result = nr.solve(&Sine, 3.0);
        assert!(result.iterations < nr.max_iterations);
        assert!((result.value - PI).abs() < nr.tolerance);

        // Find the root at x = -π
        let result = nr.solve(&Sine, -3.0);
        assert!(result.iterations < nr.max_iterations);
        assert!((result.value + PI).abs() < nr.tolerance);
    }

    #[test]
    fn test_newton_rhapson_atanh() {
        let nr = NewtonRaphson::new(100, 1e-10);

        let result = nr.solve(&Atanh { x: 0. }, 0.5);
        let expected = 0.;
        let delta = (expected - result.value).abs();
        println!("{result:?}");
        assert!(result.iterations < nr.max_iterations);
        assert!(
            delta < nr.tolerance,
            "expected: {expected}, actual: {actual} (delta: {delta})",
            actual = result.value
        );

        let result = nr.solve(&Atanh { x: 0.5 }, 0.0);
        let expected = 0.5493061443;
        let delta = (expected - result.value).abs();
        println!("{result:?}");
        assert!(result.iterations < nr.max_iterations);
        assert!(
            delta < nr.tolerance,
            "expected: {expected}, actual: {actual} (delta: {delta})",
            actual = result.value
        );
    }

    #[test]
    fn test_newton_raphson_iterations_limit() {
        // A deliberately low iteration limit
        let nr = NewtonRaphson::new(2, 1e-10);

        // This should hit the iteration limit
        let result = nr.solve(&Cubic, 5.0);
        assert_eq!(result.iterations, nr.max_iterations);

        // Check that we can still get close with enough iterations
        let nr = NewtonRaphson::new(100, 1e-10);

        let result = nr.solve(&Cubic, 5.0);
        assert!(result.iterations < nr.max_iterations);
        assert!((result.value - 4.0).abs() < nr.tolerance);
    }

    #[test]
    fn test_newton_raphson_tolerance() {
        // Test with different tolerance values
        let nr_loose = NewtonRaphson::new(100, 1e-3);

        let nr_strict = NewtonRaphson::new(100, 1e-12);

        let result_loose = nr_loose.solve(&Quadratic, 3.0);
        let result_strict = nr_strict.solve(&Quadratic, 3.0);

        // Strict tolerance should take more iterations (or equal)
        assert!(result_strict.iterations >= result_loose.iterations);

        // Both should find the root within their respective tolerances
        assert!((result_loose.value - 2.0).abs() < nr_loose.tolerance);
        assert!((result_strict.value - 2.0).abs() < nr_strict.tolerance);
    }
}
