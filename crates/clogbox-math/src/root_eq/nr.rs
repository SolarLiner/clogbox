use crate::root_eq::Differentiable;
use num_traits::Float;

/// Newton-Raphson solver
pub struct NewtonRaphson<T> {
    max_iterations: usize,
    tolerance: T,
}

pub struct SolveResult<T> {
    pub x: T,
    pub iterations: usize,
}

impl<T> NewtonRaphson<T> {
    /// Creates a new Newton-Raphson solver
    pub fn new(max_iterations: usize, tolerance: T) -> Self {
        NewtonRaphson {
            max_iterations,
            tolerance,
        }
    }

    /// Solves the equation using the Newton-Raphson method
    pub fn solve<F: Differentiable<Scalar = T>>(&self, function: &F, initial_guess: T) -> SolveResult<T>
    where
        T: Float,
    {
        let mut x = initial_guess;

        for i in 0..self.max_iterations {
            let (fx, dfx) = function.eval_with_derivative(x);

            // Newton-Raphson update
            let delta = fx / dfx;
            x = x - delta;

            // Check for convergence
            if delta.abs() < self.tolerance {
                return SolveResult { x, iterations: i };
            }
        }

        SolveResult { x, iterations: self.max_iterations }
    }
}
