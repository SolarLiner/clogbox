//! Implementation of various blocks of DSP code from the VA Filter Design book.
//!
//! Downloaded from <https://www.discodsp.net/VAFilterDesign_2.1.2.pdf>
//! All references in this module, unless specified otherwise, are taken from this book.

use crate::{Linear, Saturator};
use az::{Cast, CastFrom};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::Enum;
use clogbox_math::root_eq::nr::NewtonRaphson;
use clogbox_params::smoothers::{ExpSmoother, Smoother};
use nalgebra::{self as na, SVector, SimdRealField};
use num_traits::{Float, FloatConst, Num, NumAssign, Zero};
use numeric_literals::replace_float_literals;
use std::marker::PhantomData;
use std::ops;

mod gen;

/// Parameter type for the SVF filter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Enum)]
pub enum SvfParams {
    /// Cutoff frequency (Hz)
    Cutoff,
    /// Resonance
    Resonance,
    /// Drive
    Drive,
}

/// Represents the output types of a State Variable Filter (SVF).
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum SvfOutput {
    /// Lowpass filter output.
    Lowpass,
    /// Bandpass filter output.
    Bandpass,
    /// Highpass filter output.
    Highpass,
}

/// SVF topology filter, with optional non-linearities.
// #[derive(Debug, Clone)]
pub struct Svf<T, Mode = Linear<T>> {
    s: [T; 2],
    r: T,
    fc: T,
    g: T,
    g1: T,
    d: T,
    w_step: T,
    sample_rate: T,
    newton_raphson: NewtonRaphson<T>,
    __mode: PhantomData<Mode>,
    drive: T,
}

impl<T, Mode> Svf<T, Mode> {
    /// Replace the saturators in this SVF instance with the provided values.
    pub fn with_saturator<S2>(self) -> Svf<T, S2> {
        let Self {
            s,
            r,
            fc,
            g,
            g1,
            d,
            w_step,
            sample_rate,
            newton_raphson,
            drive,
            ..
        } = self;
        Svf {
            s,
            r,
            fc,
            g,
            g1,
            d,
            w_step,
            sample_rate,
            newton_raphson,
            drive,
            __mode: PhantomData,
        }
    }
}

impl<T: Copy + Float + FloatConst + CastFrom<f64> + Num, Mode> Svf<T, Mode> {
    /// Create a new SVF filter with the provided sample rate, frequency cutoff (in Hz) and resonance amount
    /// (in 0..1 for stable filters, otherwise use nonlinearities).
    #[replace_float_literals(T::cast_from(literal))]
    pub fn new(sample_rate: T, cutoff: f32, resonance: f32) -> Self {
        let fc = T::cast_from(cutoff as _);
        let q = T::cast_from(resonance as _);
        let mut this = Self {
            s: [0.; 2],
            r: 1. - q,
            fc,
            g: 0.,
            g1: 0.,
            d: 0.,
            sample_rate,
            w_step: T::PI() / sample_rate,
            newton_raphson: NewtonRaphson {
                over_relaxation: 1.0,
                max_iterations: 10,
                tolerance: 1e-4,
            },
            drive: 1.0,
            __mode: PhantomData,
        };
        this.update_coefficients();
        this
    }
}
impl<T: Cast<f64> + CastFrom<f64> + Float, Mode> Svf<T, Mode> {
    /// Set the new filter cutoff frequency (in Hz).
    pub fn set_cutoff(&mut self, freq: T) {
        self.set_cutoff_no_update(freq);
        self.update_coefficients();
    }

    /// Sets the cutoff frequency (in Hz) without triggering a recomputation of internal parameters.
    /// It's required to call [`Self::update_coefficients`] after a call to this, and before
    /// processing the next sample.
    pub fn set_cutoff_no_update(&mut self, freq: T) {
        self.fc = freq;
    }

    /// Set the resonance amount (in 0..1 for stable filters, otherwise use bounded nonlinearities).
    pub fn set_resonance(&mut self, r: T) {
        self.set_resonance_no_update(r);
        self.update_coefficients();
    }

    /// Sets the cutoff frequency (in Hz) without triggering a recomputation of internal parameters.
    /// It's required to call [`Self::update_coefficients`] after a call to this, and before
    /// processing the next sample.
    #[replace_float_literals(T::cast_from(literal))]
    pub fn set_resonance_no_update(&mut self, r: T) {
        // let r = 1. - r * resonance_compensation(self.fc / self.sample_rate);
        let r = 1. - r;
        self.r = 2. * r;
    }
    
    pub fn set_drive(&mut self, drive: T) {
        self.drive = drive;
    }

    pub fn set_newton_rhapson(&mut self, newton_raphson: NewtonRaphson<T>) {
        self.newton_raphson = newton_raphson;
    }

    pub fn with_newton_rhapson(mut self, newton_raphson: NewtonRaphson<T>) -> Self {
        self.newton_raphson = newton_raphson;
        self
    }
}

#[replace_float_literals(T::cast_from(literal))]
fn resonance_compensation<T: CastFrom<f64> + Float>(x: T) -> T {
    (0.99 + x.tan()).recip()
}

impl<T: Float + CastFrom<f64>, Mode> Svf<T, Mode> {
    /// Recompute the internal parameters from the cutoff and resonance parameters.
    #[profiling::function]
    #[replace_float_literals(T::cast_from(literal))]
    pub fn update_coefficients(&mut self) {
        self.g = self.w_step * self.fc;
        self.g1 = 2. * self.r + self.g;
        self.d = (1. + 2. * self.r * self.g + self.g * self.g).recip();
    }
}

impl<T: Copy + Float + FloatConst + CastFrom<f64>> Svf<T, Linear<T>> {
    #[replace_float_literals(T::cast_from(literal))]
    pub fn next_sample(&mut self, x: T) -> EnumMapArray<SvfOutput, T> {
        let [s1, s2] = self.s;

        let bpp = s1;
        let bpl = (self.r - 1.) * s1;
        let bp1 = 2. * (bpp + bpl);
        let hp = (x - bp1 - s2) * self.d;

        let v1 = self.g * hp;
        let bp = v1 + s1;
        let s1 = bp + v1;

        let v2 = self.g * bp;
        let lp = v2 + s2;
        let s2 = lp + v2;

        self.s = [s1, s2];

        EnumMapArray::new(|ch| match ch {
            SvfOutput::Lowpass => lp,
            SvfOutput::Bandpass => bp,
            SvfOutput::Highpass => hp,
        })
    }
}

/// Marker trait for SVF filters with OTA non-linearities
pub struct Ota;

impl<
        T: 'static
            + Copy
            + Send
            + Cast<f64>
            + CastFrom<f64>
            + Float
            + na::RealField
            + FloatConst
            + na::Scalar
            + SimdRealField
            + NumAssign,
    > Svf<T, Ota>
{
    /// Process the next sample of this filter, with the given input sample.
    ///
    /// The output samples are `(LP, BP, HP)`
    #[replace_float_literals(T::cast_from(literal))]
    pub fn next_sample(&mut self, x: T) -> EnumMapArray<SvfOutput, T> {
        let eq = gen::SvfEquation {
            g: self.g,
            x,
            R: self.r,
            S: na::OVector::from(self.s),
            k_drive: self.drive,
        };
        let mut x = SVector::<T, 3>::new(x, self.s[0], self.s[1]);
        self.newton_raphson.solve_multi(&eq, x.as_view_mut());

        let s = gen::state(na::OVector::from(self.s), self.g, x[1], x[2], x[0]);
        self.s = s.into();
        EnumMapArray::new(|ch| match ch {
            SvfOutput::Lowpass => x[0],
            SvfOutput::Bandpass => x[1],
            SvfOutput::Highpass => x[2],
        })
    }

    /// Set a parameter using the [`SvfParams`] [`Enum`].
    pub fn set_param(&mut self, param: SvfParams, value: f32) {
        match param {
            SvfParams::Cutoff => self.set_cutoff(T::cast_from(value as _)),
            SvfParams::Resonance => self.set_resonance(T::cast_from(value as _)),
            SvfParams::Drive => self.drive = T::cast_from(value as _),       
        }
    }
}

/// Enum representing different types of audio filters.
#[derive(Debug, Copy, Clone, Enum, Eq, PartialEq, Ord, PartialOrd)]
pub enum FilterType {
    /// No filtering, signal is passed unchanged.
    Bypass,
    /// Low pass filter.
    #[display = "Low pass"]
    Lowpass,
    /// Band pass filter.
    #[display = "Band pass"]
    Bandpass,
    /// High pass filter.
    #[display = "High pass"]
    Highpass,
    /// Low shelf filter.
    #[display = "Low shelf"]
    Lowshelf,
    /// High shelf filter.
    #[display = "High shelf"]
    Highshelf,
    /// Peak (Sharp) filter.
    #[display = "Peak (Sharp)"]
    PeakSharp,
    /// Peak (Shelf) filter.
    #[display = "Peak (Shelf)"]
    PeakShelf,
    /// Notch filter.
    Notch,
    /// All-pass filter.
    #[display = "All-pass"]
    Allpass,
}

impl FilterType {
    /// Computes the mixing coefficients for the filter type based on the provided amplitude.
    #[replace_float_literals(T::cast_from(literal))]
    pub fn mix_coefficients<T: ops::Neg<Output = T> + ops::Sub<Output = T> + CastFrom<f64>>(&self, amp: T) -> [T; 4] {
        let g = amp - 1.0;
        match self {
            Self::Bypass => [1.0, 0.0, 0.0, 0.0],
            Self::Lowpass => [0.0, 1.0, 0.0, 0.0],
            Self::Bandpass => [0.0, 0.0, 1.0, 0.0],
            Self::Highpass => [0.0, 0.0, 0.0, 1.0],
            Self::Lowshelf => [1.0, g, 0.0, 0.0],
            Self::Highshelf => [1.0, 0.0, 0.0, g],
            Self::PeakSharp => [0.0, 1.0, 0.0, -1.0],
            Self::PeakShelf => [1.0, 0.0, g, 0.0],
            Self::Notch => [1.0, 0.0, -1.0, 1.0],
            Self::Allpass => [1.0, 0.0, -2.0, 0.0],
        }
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum SvfMixerParams {
    #[display = "Filter Type"]
    FilterType,
    Amplitude,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum SvfMixerInput {
    AudioInput,
    SvfOutput(SvfOutput),
    Params(SvfMixerParams),
}

#[derive(Debug, Copy, Clone)]
pub struct SvfMixer<T> {
    filter_type: FilterType,
    amp: ExpSmoother<T>,
    coeffs: [ExpSmoother<T>; 4],
}

impl<T: 'static + Copy + Send + Float + NumAssign + ops::Neg<Output = T> + CastFrom<f64>> SvfMixer<T> {
    pub fn mix(&mut self, input: T, outputs: EnumMapArray<SvfOutput, T>) -> T {
        use SvfOutput::*;
        let k = self.coeffs.each_mut().map(|s| s.next_value());
        let x = [input, outputs[Lowpass], outputs[Bandpass], outputs[Highpass]];
        k.into_iter()
            .zip(x)
            .map(|(k, x)| k * x)
            .reduce(ops::Add::add)
            .unwrap_or(T::zero())
    }
}
