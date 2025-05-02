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
use nalgebra::{self as na, SimdRealField};
use num_traits::{Float, FloatConst, Num, NumAssign, Zero};
use numeric_literals::replace_float_literals;
use std::marker::PhantomData;
use std::ops;

/// Output given by the implementations of the SVF filter signal path
pub struct SvfSampleOutput<T> {
    /// Filter output (LP, BP, HP)
    pub y: [T; 3],
    /// State
    pub s: [T; 2],
}

/// Trait for SVF filter implementations.
pub trait SvfImpl<T> {
    /// Compute the next sample for outputs and state
    fn next_sample(svf: &mut Svf<T, Self>, input: T) -> SvfSampleOutput<T>;
}

impl<T: Float + az::CastFrom<f64>> SvfImpl<T> for Linear<T> {
    #[replace_float_literals(T::cast_from(literal))]
    #[inline]
    fn next_sample(svf: &mut Svf<T, Self>, input: T) -> SvfSampleOutput<T> {
        let [s1, s2] = svf.s;

        let bpp = s1;
        let bpl = (svf.q - 1.) * s1;
        let bp1 = 2. * (bpp + bpl);
        let hp = (input - bp1 - s2) * svf.d;

        let v1 = svf.g * hp;
        let bp = v1 + s1;
        let s1 = bp + v1;

        let v2 = svf.g * bp;
        let lp = v2 + s2;
        let s2 = lp + v2;

        SvfSampleOutput {
            y: [lp, bp, hp],
            s: [s1, s2],
        }
    }
}

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
pub struct Svf<T, Mode: ?Sized = Linear<T>> {
    /// Inner state of the filter
    pub s: [T; 2],
    /// Last output of the filter
    pub last_out: [T; 3],
    /// Filter resonance (0..1)
    pub q: T,
    /// Filter cutoff frequency (in Hz)
    pub fc: T,
    /// Computed normalized frequency
    pub g: T,
    /// Pre-computed input amplitude to the first integrator 
    pub d: T,
    /// Radian step (where 1 radian = 1/sample_rate)
    pub w_step: T,
    /// Sample rate (Hz)
    pub sample_rate: T,
    /// Filter drive (does not drive the input, instead drives the nonlinearities directly)
    pub drive: T,
    __mode: PhantomData<Mode>,
}

impl<T, Mode> Svf<T, Mode> {
    /// Replace the saturators in this SVF instance with the provided values.
    pub fn with_mode<Mode2>(self) -> Svf<T, Mode2> {
        let Self {
            s,
            last_out,
            q,
            fc,
            g,
            d,
            w_step,
            sample_rate,
            drive,
            ..
        } = self;
        Svf {
            s,
            last_out,
            q,
            fc,
            g,
            d,
            w_step,
            sample_rate,
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
            last_out: [0.; 3],
            q,
            fc,
            g: 0.,
            d: 0.,
            sample_rate,
            w_step: T::PI() / sample_rate,
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
    pub fn set_resonance_no_update(&mut self, q: T) {
        self.q = q;
    }
    
    pub fn set_drive(&mut self, drive: T) {
        self.drive = drive;
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
        self.d = (1. + 2. * self.q * self.g + self.g * self.g).recip();
    }
}

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
    Mode: SvfImpl<T>,
    > Svf<T, Mode>
{
    /// Process the next sample of this filter, with the given input sample.
    ///
    /// The output samples are `(LP, BP, HP)`
    #[replace_float_literals(T::cast_from(literal))]
    pub fn next_sample(&mut self, x: T) -> EnumMapArray<SvfOutput, T> {
        let SvfSampleOutput { mut y, mut s } = Mode::next_sample(self, x);
        for x in &mut y {
            if x.is_nan() {
                x.set_zero();
            }
        }
        for i in 0..2 {
            if !s[i].is_nan() {
                self.s[i] = s[i];
            }
        }
        EnumMapArray::new(|ch| match ch {
            SvfOutput::Lowpass => y[0],
            SvfOutput::Bandpass => y[1],
            SvfOutput::Highpass => y[2],
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
