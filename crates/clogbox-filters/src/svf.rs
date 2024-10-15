//! Implementation of various blocks of DSP code from the VA Filter Design book.
//!
//! Downloaded from <https://www.discodsp.net/VAFilterDesign_2.1.2.pdf>
//! All references in this module, unless specified otherwise, are taken from this book.

use crate::{Linear, Saturator};
use az::CastFrom;
use clogbox_core::module::analysis::{FreqAnalysis, Matrix};
use clogbox_core::module::sample::{ModuleContext, SampleModule};
use clogbox_core::module::ProcessStatus;
use clogbox_core::r#enum::Enum;
use clogbox_derive::Enum;
use generic_array::{ArrayLength, GenericArray};
use num_complex::Complex;
use num_traits::{Float, FloatConst, Num, One};
use numeric_literals::replace_float_literals;

/// Parameter type for the SVF filter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Enum)]
pub enum SvfParams {
    /// Cutoff frequency (Hz)
    Cutoff,
    /// Resonance
    Resonance,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum SvfInput {
    AudioInput,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum SvfOutput {
    Lowpass,
    Bandpass,
    Highpass,
}

/// SVF topology filter, with optional non-linearities.
#[derive(Debug, Copy, Clone)]
pub struct Svf<T, Mode = Linear<T>> {
    s: [T; 2],
    r: T,
    fc: T,
    g: T,
    g1: T,
    d: T,
    w_step: T,
    samplerate: T,
    saturator: Mode,
}

impl<T, Mode> Svf<T, Mode> {
    /// Apply these new saturators to this SVF instance, returning a new instance of it.
    pub fn set_saturator(&mut self, saturator: Mode) {
        self.saturator = saturator;
    }

    /// Replace the saturators in this Biquad instance with the provided values.
    pub fn with_saturator<S2>(self, saturator: S2) -> Svf<T, S2> {
        let Self {
            s,
            r,
            fc,
            g,
            g1,
            d,
            w_step,
            samplerate,
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
            samplerate,
            saturator,
        }
    }
}

impl<T: Copy + FloatConst + CastFrom<f64> + Num> Svf<T, Linear<T>> {
    /// Create a new SVF filter with the provided sample rate, frequency cutoff (in Hz) and resonance amount
    /// (in 0..1 for stable filters, otherwise use bounded nonlinearities).
    #[replace_float_literals(T::cast_from(literal))]
    pub fn new(samplerate: T, cutoff: T, resonance: T) -> Self {
        let mut this = Self {
            s: [0.; 2],
            r: 1. - resonance,
            fc: cutoff,
            g: 0.,
            g1: 0.,
            d: 0.,
            samplerate,
            w_step: T::PI() / samplerate,
            saturator: Linear::default(),
        };
        this.update_coefficients();
        this
    }
}
impl<T: Copy + CastFrom<f64> + Num, C> Svf<T, C> {
    /// Set the new filter cutoff frequency (in Hz).
    pub fn set_cutoff(&mut self, freq: T) {
        self.fc = freq;
        self.update_coefficients();
    }

    /// Set the resonance amount (in 0..1 for stable filters, otherwise use bounded nonlinearities).
    #[replace_float_literals(T::cast_from(literal))]
    pub fn set_r(&mut self, r: T) {
        let r = 1. - r;
        self.r = 2. * r;
        self.update_coefficients();
    }

    #[profiling::function]
    #[replace_float_literals(T::cast_from(literal))]
    fn update_coefficients(&mut self) {
        self.g = self.w_step * self.fc;
        self.g1 = 2. * self.r + self.g;
        self.d = (1. + 2. * self.r * self.g + self.g * self.g).simd_recip();
    }
}

impl<T: Copy + CastFrom<f64> + Num, Mode: Saturator<Sample = T>> SampleModule for Svf<T, Mode> {
    type Sample = Mode::Sample;
    type Inputs = SvfInput;
    type Outputs = SvfOutput;

    fn reset(&mut self) {
        self.s.fill(T::cast_from(0.));
    }

    #[replace_float_literals(T::cast_from(literal))]
    fn latency(&self) -> GenericArray<f64, SvfOutput::Count>
    where
        SvfOutput::Count: ArrayLength,
    {
        let k = self.fc / self.samplerate;
        GenericArray::<T, SvfOutput::Count>::from_array([2. * (1. - k), 1., 2. * k])
    }

    #[replace_float_literals(T::cast_from(literal))]
    fn process(&mut self, context: &mut ModuleContext<Self>) -> ProcessStatus
    where
        <Self::Inputs as Enum>::Count: ArrayLength,
        <Self::Outputs as Enum>::Count: ArrayLength,
    {
        let x = context.inputs[SvfInput::AudioInput];
        let [s1, s2] = self.s;

        let bpp = self.saturator.saturate(s1);
        let bpl = (self.r - 1.) * s1;
        let bp1 = 2. * (bpp + bpl);
        let hp = (x - bp1 - s2) * self.d;
        self.saturator.update_state(s1, bpp);

        let v1 = self.g * hp;
        let bp = v1 + s1;
        let s1 = bp + v1;

        let v2 = self.g * bp;
        let lp = v2 + s2;
        let s2 = lp + v2;

        self.s = [s1, s2];
        context.outputs[SvfOutput::Lowpass] = lp;
        context.outputs[SvfOutput::Bandpass] = bp;
        context.outputs[SvfOutput::Highpass] = hp;
        ProcessStatus::Tail(2)
    }
}

impl<T: Copy + CastFrom<f64> + Num, Mode: Saturator<Sample = T>> FreqAnalysis for Svf<T, Mode> {
    #[replace_float_literals(Complex::from(T::cast_from(literal)))]
    fn h_z(
        &self,
        z: Complex<Self::Sample>,
    ) -> Matrix<Self::Sample, <Self::Outputs as Enum>::Count, <Self::Inputs as Enum>::Count> {
        let omega_c = 2.0 * self.fc * self.w_step;
        let x0 = z + 1.0;
        let x1 = x0.powi(2) * omega_c.simd_powi(2);
        let x2 = z - 1.0;
        let x3 = x2.powi(2) * 4.0;
        let x4 = x0 * x2 * omega_c;
        let x5 = (-x4 * 4.0 * self.r + x1 + x3).recip();
        [[x1 * x5, -x4 * x5 * 2.0, x3 * x5]]
    }
}

#[derive(Debug, Copy, Clone, Enum, Eq, PartialEq)]
pub enum FilterType {
    Bypass,
    #[r#enum(display = "Low pass")]
    Lowpass,
    #[r#enum(display = "Band pass")]
    Bandpass,
    #[r#enum(display = "High pass")]
    Highpass,
    #[r#enum(display = "Low shelf")]
    Lowshelf,
    #[r#enum(display = "High shelf")]
    Highshelf,
    #[r#enum(display = "Peak (Sharp)")]
    PeakSharp,
    #[r#enum(display = "Peak (Shelf)")]
    PeakShelf,
    Notch,
    #[r#enum(display = "All-pass")]
    Allpass,
}

impl FilterType {
    pub fn mix_coefficients<T: Float>(&self, amp: T) -> [T; 4] {
        let g = amp - T::one();
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