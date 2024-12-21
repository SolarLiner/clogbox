use crate::math::recip::Recip;
use az::CastFrom;
use num_traits::{Float, NumAssign};
use numeric_literals::replace_float_literals;

pub trait Smoother<T> {
    /// Computes the next value in the smoothing process.
    ///
    /// # Returns
    ///
    /// The next smoothed value.
    fn next_value(&mut self) -> T;
    fn has_converged(&self) -> bool;
    fn set_target(&mut self, target: T);
    fn next_buffer(&mut self, buffer: &mut [T]) {
        for value in buffer {
            *value = self.next_value();
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct LinearSmoother<T> {
    value: T,
    target: T,
    step: T,
}

impl<T: Float> LinearSmoother<T> {
    /// Creates a new `LinearSmoother` instance.
    ///
    /// # Parameters
    ///
    /// - `value`: The initial value.
    /// - `target`: The target value to smooth towards.
    /// - `samplerate`: The sample rate for smoothing.
    ///
    /// # Returns
    ///
    /// A new `LinearSmoother` instance.
    pub fn new(value: T, target: T, speed: T, samplerate: impl Into<Recip<T>>) -> Self {
        Self {
            value,
            target,
            step: speed * samplerate.into().recip(),
        }
    }
}

impl<T: Float + NumAssign> Smoother<T> for LinearSmoother<T> {
    fn next_value(&mut self) -> T {
        if !self.has_converged() {
            self.value += self.step;
            if self.has_converged()
                || self.value * self.step.signum() > self.target * self.step.signum()
            {
                self.value = self.target;
            }
        }
        self.value
    }
    fn has_converged(&self) -> bool {
        (self.value - self.target).abs() < self.step
    }
    fn set_target(&mut self, target: T) {
        self.target = target;
        self.step = self.step.copysign(target - self.value);
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ExponentialSmoother<T> {
    value: T,
    target: T,
    tau: T,
}

impl<T: Float + NumAssign + CastFrom<f64>> ExponentialSmoother<T> {
    /// Creates a new `ExponentialSmoother` instance.
    ///
    /// # Parameters
    ///
    /// - `value`: The initial value.
    /// - `target`: The target value to smooth towards.
    /// - `tau`: The time constant for smoothing.
    /// - `samplerate`: The sample rate for smoothing.
    ///
    /// # Returns
    ///
    /// A new `ExponentialSmoother` instance.
    pub fn new(value: T, target: T, time: T, samplerate: impl Into<Recip<T>>) -> Self {
        Self {
            value,
            target,
            tau: Self::tau(time, samplerate.into().recip()),
        }
    }

    /// Computes the next value in the smoothing process.
    ///
    /// # Returns
    ///
    /// The next smoothed value.
    pub fn next_value(&mut self) -> T {
        self.value += self.tau * (self.value - self.target);
        self.value
    }

    pub fn next_buffer(&mut self, buffer: &mut [T]) {
        for value in buffer {
            *value = self.next_value();
        }
    }

    /// Sets a new target value for the smoother.
    ///
    /// # Parameters
    ///
    /// - `target`: The new target value.
    pub fn set_target(&mut self, target: T) {
        self.target = target;
    }

    /// Checks if the smoother has converged to the target value.
    #[replace_float_literals(T::cast_from(literal))]
    pub fn has_converged(&self) -> bool {
        (self.value - self.target).abs() < 1e-6
    }

    fn tau(time: T, dt: T) -> T {
        const T60: f64 = 6.91;
        -dt / (time * T::cast_from(T60))
    }
}

#[derive(Debug, Copy, Clone)]
pub enum OptionalSmoother<T, S: Smoother<T> = ExponentialSmoother<T>> {
    Smoothed(S),
    Unsmoothed(T),
}

impl<T, S: Smoother<T>> OptionalSmoother<T, S> {
    pub fn unsmoothed(value: T) -> Self {
        Self::Unsmoothed(value)
    }
}

impl<T: Copy + Float + NumAssign> OptionalSmoother<T, LinearSmoother<T>> {
    pub fn linear(samplerate: T, value: T, speed: T) -> Self {
        Self::Smoothed(LinearSmoother::new(value, value, speed, samplerate))
    }
}

impl<T: Copy + Float + NumAssign + CastFrom<f64>> OptionalSmoother<T, ExponentialSmoother<T>>
where
    ExponentialSmoother<T>: Smoother<T>,
{
    pub fn exponential(samplerate: T, value: T, time: T) -> Self {
        Self::Smoothed(ExponentialSmoother::new(value, value, time, samplerate))
    }
}

impl<T: Copy, S: Smoother<T>> Smoother<T> for OptionalSmoother<T, S> {
    fn next_value(&mut self) -> T {
        match self {
            Self::Smoothed(s) => s.next_value(),
            Self::Unsmoothed(v) => *v,
        }
    }

    fn has_converged(&self) -> bool {
        match self {
            Self::Smoothed(s) => s.has_converged(),
            Self::Unsmoothed(_) => true,
        }
    }

    fn set_target(&mut self, target: T) {
        match self {
            Self::Smoothed(s) => s.set_target(target),
            Self::Unsmoothed(v) => *v = target,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_smoother() {
        let mut smoother = LinearSmoother::new(0.0, 1.0, 0.1, 1.0);
        let mut array = [0.0; 10];
        smoother.next_buffer(&mut array);
        insta::assert_csv_snapshot!(array, { "[]" => insta::rounded_redaction(4) });
    }

    #[test]
    fn test_exponential_smoother() {
        let mut smoother = ExponentialSmoother::new(0.0, 1.0, 0.04, 10.);
        let mut array = [0.0; 11];
        smoother.next_buffer(&mut array);
        insta::assert_csv_snapshot!(array, { "[]" => insta::rounded_redaction(4) });
    }
}
