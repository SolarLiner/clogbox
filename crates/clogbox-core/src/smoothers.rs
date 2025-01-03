use crate::math::interpolation::{InterpolateSingle, Linear};
use num_traits::{Float, Num, NumAssign, NumOps, One, Zero};
use numeric_array::NumericArray;

pub trait Smoother<T> {
    fn next_value(&mut self) -> T;
    fn has_converged(&self) -> bool;
    fn set_target(&mut self, target: T);
}

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

impl<T: Copy + Zero + Num, Interp> InterpSmoother<T, Interp> {
    pub fn new(interp: Interp, samplerate: T, time: T, initial: T, target: T) -> Self {
        Self {
            f: T::zero(),
            step: samplerate / time,
            initial,
            target,
            time,
            samplerate,
            interp,
        }
    }
}

impl<T: Copy + Float + az::Cast<usize>, Interp: InterpolateSingle<T>> Smoother<T>
    for InterpSmoother<T, Interp>
{
    fn next_value(&mut self) -> T {
        if self.has_converged() {
            self.target
        } else {
            let x = self
                .interp
                .interpolate_single(NumericArray::from_slice(&[T::zero(), T::one()]), self.f);
            let out = Linear.interpolate_single(NumericArray::from_slice(&[self.initial, self.target]), x);
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

pub type LinearSmoother<T> = InterpSmoother<T, Linear>;

#[derive(Debug, Copy, Clone)]
pub struct ExpSmoother<T> {
    target: T,
    tau: T,
    time: T,
    samplerate: T,
    last: T,
}

impl<T: Copy + NumOps> ExpSmoother<T> {
    pub fn new(samplerate: T, time: T, initial: T, target: T) -> Self {
        Self {
            target,
            tau: time / samplerate,
            time,
            samplerate,
            last: initial,
        }
    }
}

impl<T: az::CastFrom<f64> + Float + NumAssign> Smoother<T> for ExpSmoother<T> {
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
