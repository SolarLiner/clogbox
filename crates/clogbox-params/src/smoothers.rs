//! Parameter smoothing implementations.
//!
//! This module provides various parameter smoothing algorithms that can be used
//! to gradually change parameter values over time, preventing clicks and other
//! artifacts in the audio signal.
use clogbox_math::interpolation::{InterpolateSingle, Linear};
use num_traits::{Float, NumAssign, NumOps, One};
use numeric_array::NumericArray;

/// Trait for parameter smoothing algorithms.
///
/// This trait defines the interface for parameter smoothers, which gradually
/// change a parameter value over time to prevent audio artifacts.
pub trait Smoother<T> {
    /// Returns the current smoothed value.
    ///
    /// This method returns the current value without advancing the smoother.
    fn current_value(&self) -> T;

    /// Advances the smoother and returns the next smoothed value.
    ///
    /// This method calculates the next value in the smoothing sequence and
    /// updates the internal state of the smoother.
    fn next_value(&mut self) -> T;

    /// Checks if the smoother has reached its target value.
    ///
    /// Returns `true` if the current value is close enough to the target
    /// value that further smoothing is unnecessary.
    fn has_converged(&self) -> bool;

    /// Sets a new target value for the smoother.
    ///
    /// This method changes the target value that the smoother will
    /// gradually approach over time.
    ///
    /// # Parameters
    ///
    /// * `target` - The new target value
    fn set_target(&mut self, target: T);
}

/// A parameter smoother that uses interpolation between values.
///
/// This smoother uses a specified interpolation algorithm to smoothly
/// transition between an initial value and a target value over a given time.
#[derive(Debug, Copy, Clone)]
pub struct InterpSmoother<T, Interp> {
    f: T,
    step: T,
    initial: T,
    target: T,
    time: T,
    samplerate: T,
    interp: Interp,
}

impl<T: Copy + Float, Interp> InterpSmoother<T, Interp> {
    /// Creates a new interpolation-based smoother.
    ///
    /// # Parameters
    ///
    /// * `interp` - The interpolation algorithm to use
    /// * `samplerate` - The sample rate in Hz
    /// * `time` - The smoothing time in seconds
    /// * `initial` - The initial value
    /// * `target` - The target value
    ///
    /// # Returns
    ///
    /// A new `InterpSmoother` instance
    pub fn new(interp: Interp, samplerate: T, time: T, initial: T, target: T) -> Self {
        Self {
            f: T::zero(),
            step: (time * samplerate).recip(),
            initial,
            target,
            time,
            samplerate,
            interp,
        }
    }

    /// Sets a new samplerate for this smoother.
    ///
    /// # Arguments
    ///
    /// * `samplerate`: New sample rate (Hz)
    pub fn set_samplerate(&mut self, samplerate: T) {
        self.samplerate = samplerate;
        self.step = (self.time * samplerate).recip();
    }
}

impl<T: Copy + Float + az::Cast<usize>, Interp: InterpolateSingle<T>> Smoother<T> for InterpSmoother<T, Interp> {
    fn current_value(&self) -> T {
        self.interp
            .interpolate_single(NumericArray::from_slice(&[self.initial, self.target]), self.f)
    }

    fn next_value(&mut self) -> T {
        if self.has_converged() {
            self.target
        } else {
            let out = Linear.interpolate_single(NumericArray::from_slice(&[self.initial, self.target]), self.f);
            self.f = T::clamp(self.f + self.step, T::zero(), T::one());
            out
        }
    }

    fn has_converged(&self) -> bool {
        self.f >= T::one()
    }

    fn set_target(&mut self, target: T) {
        self.initial = self.next_value();
        self.target = target;
        self.f = T::zero();
    }
}

/// A linear interpolation smoother.
///
/// This is a type alias for `InterpSmoother` using linear interpolation,
/// which provides a simple linear transition between values.
pub type LinearSmoother<T> = InterpSmoother<T, Linear>;

/// An exponential parameter smoother.
///
/// This smoother uses an exponential decay function to smoothly
/// transition between values, providing a more natural-sounding
/// transition for many audio parameters.
#[derive(Debug, Copy, Clone)]
pub struct ExpSmoother<T> {
    target: T,
    tau: T,
    time: T,
    samplerate: T,
    last: T,
}

impl<T: Copy + One + NumOps> ExpSmoother<T> {
    /// Creates a new exponential smoother.
    ///
    /// # Parameters
    ///
    /// * `samplerate` - The sample rate in Hz
    /// * `time` - The smoothing time in seconds
    /// * `initial` - The initial value
    /// * `target` - The target value
    ///
    /// # Returns
    ///
    /// A new `ExpSmoother` instance
    pub fn new(samplerate: T, time: T, initial: T, target: T) -> Self {
        Self {
            target,
            tau: T::one() - time / samplerate,
            time,
            samplerate,
            last: initial,
        }
    }

    /// Sets the new samplerate of the smoother.
    ///
    /// # Arguments
    ///
    /// * `samplerate`: Sample rate (Hz)
    pub fn set_samplerate(&mut self, samplerate: T) {
        self.samplerate = samplerate;
        self.tau = T::one() - self.time / samplerate;
    }
}

impl<T: az::CastFrom<f64> + Float + NumAssign> Smoother<T> for ExpSmoother<T> {
    fn current_value(&self) -> T {
        self.last
    }

    fn next_value(&mut self) -> T {
        self.last += self.tau * (self.target - self.last);
        self.last
    }

    fn has_converged(&self) -> bool {
        (self.last - self.target).abs() < T::cast_from(1e-6)
    }

    fn set_target(&mut self, target: T) {
        self.target = target;
    }
}
