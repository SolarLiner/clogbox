//! Implementation of various blocks of DSP code from the VA Filter Design book.
//!
//! Downloaded from <https://www.discodsp.net/VAFilterDesign_2.1.2.pdf>
//! All references in this module, unless specified otherwise, are taken from this book.

use crate::{Linear, Saturator};
use az::{Cast, CastFrom};
use clogbox_core::module::analysis::{FreqAnalysis, Matrix};
use clogbox_core::module::sample::SampleModule;
use clogbox_core::module::{ProcessStatus, StreamData};
use clogbox_core::param::{FloatMapping, Value, FloatRange, Params, IValue};
use clogbox_core::r#enum::enum_map::{EnumMapArray, EnumMapMut};
use clogbox_core::r#enum::Enum;
use clogbox_derive::Enum;
use generic_array::GenericArray;
use num_complex::Complex;
use num_traits::{Float, FloatConst, Num, Zero};
use numeric_array::NumericArray;
use numeric_literals::replace_float_literals;
use std::ops;
use std::sync::Arc;

/// Parameter type for the SVF filter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Enum)]
pub enum SvfParams {
    /// Cutoff frequency (Hz)
    Cutoff,
    /// Resonance
    Resonance,
}

/// Represents the different inputs for the SVF (state variable filter).
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum SvfInput {
    /// Audio input for the SVF.
    AudioInput,
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
#[derive(Debug, Clone)]
pub struct Svf<T, Mode = Linear<T>> {
    s: [T; 2],
    r: T,
    fc: T,
    g: T,
    g1: T,
    d: T,
    w_step: T,
    sample_rate: T,
    saturator: Mode,
    params: Arc<EnumMapArray<SvfParams, Value>>,
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
            sample_rate,
            params,
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
            saturator,
            params,
        }
    }
}

impl<T: Copy + Float + FloatConst + CastFrom<f64> + Num> Svf<T, Linear<T>> {
    const fn cutoff_param(cutoff: f32) -> Value {
        Value::new(cutoff)
            .with_range(FloatRange::new(20.0..=20e3).with_mapping(FloatMapping::Logarithmic))
    }

    const fn resonance_param(resonance: f32) -> Value {
        Value::new(resonance)
            .with_range(FloatRange::new(0.0..=1.25).with_mapping(FloatMapping::Logarithmic))
    }

    /// Create a new SVF filter with the provided sample rate, frequency cutoff (in Hz) and resonance amount
    /// (in 0..1 for stable filters, otherwise use bounded nonlinearities).
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
            saturator: Linear::default(),
            params: Arc::new(EnumMapArray::from_std_array([
                Self::cutoff_param(cutoff),
                Self::resonance_param(resonance),
            ])),
        };
        this.update_coefficients();
        this
    }
}
impl<T: Cast<f64> + CastFrom<f64> + Float, C> Svf<T, C> {
    /// Set the new filter cutoff frequency (in Hz).
    pub fn set_cutoff(&mut self, freq: T) {
        self.fc = freq;
        self.update_coefficients();
    }
}
impl<T: Copy + CastFrom<f64> + Cast<f64> + Float, C> Svf<T, C> {
    /// Set the resonance amount (in 0..1 for stable filters, otherwise use bounded nonlinearities).
    #[replace_float_literals(T::cast_from(literal))]
    pub fn set_resonance(&mut self, r: T) {
        let r = 1. - r;
        self.r = 2. * r;
        self.update_coefficients();
    }
}

impl<T: Float + CastFrom<f64>, C> Svf<T, C> {
    #[profiling::function]
    #[replace_float_literals(T::cast_from(literal))]
    fn update_coefficients(&mut self) {
        self.g = self.w_step * self.fc;
        self.g1 = 2. * self.r + self.g;
        self.d = (1. + 2. * self.r * self.g + self.g * self.g).recip();
    }
}

impl<
        T: 'static + Send + Cast<f64> + CastFrom<f64> + Float,
        Mode: 'static + Send + Saturator<Sample = T>,
    > SampleModule for Svf<T, Mode>
{
    type Sample = Mode::Sample;
    type Inputs = SvfInput;
    type Outputs = SvfOutput;
    type Params = SvfParams;

    fn get_params(&self) -> Arc<impl '_ + Params<Params= Self::Params>> {
        self.params.clone()
    }

    fn reset(&mut self) {
        self.s.fill(T::cast_from(0.));
    }

    fn latency(
        &self,
        input_latency: EnumMapArray<Self::Inputs, f64>,
    ) -> EnumMapArray<Self::Outputs, f64> {
        let l = input_latency[SvfInput::AudioInput];
        let k = f64::cast_from(self.fc / self.sample_rate);
        EnumMapArray::from_array([2. * (1. - k), 1., 2. * k].map(|x| l + x).into())
    }

    #[replace_float_literals(T::cast_from(literal))]
    fn process_sample(
        &mut self,
        _: &StreamData,
        inputs: EnumMapArray<Self::Inputs, Self::Sample>,
        mut outputs: EnumMapMut<Self::Outputs, Self::Sample>,
    ) -> ProcessStatus {
        use SvfInput::*;
        use SvfOutput::*;
        let x = inputs[AudioInput];
        let [s1, s2] = self.s;

        let bpp = self.saturator.saturate(s1);
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
        outputs[Lowpass] = lp;
        outputs[Bandpass] = bp;
        outputs[Highpass] = hp;
        ProcessStatus::Running
    }

    fn on_begin_block(&mut self, stream_data: &StreamData) -> ProcessStatus {
        use SvfParams::*;
        if self.params[Cutoff].has_changed() {
            let value = T::cast_from(self.params[Cutoff].get_value() as _);
            self.set_cutoff(value);
        }
        if self.params[Resonance].has_changed() {
            let value = T::cast_from(self.params[Resonance].get_value() as _);
            self.set_resonance(value);
        }
        ProcessStatus::Running
    }
}

impl<
        T: 'static + Send + Copy + Zero + CastFrom<f64> + Cast<f64> + Float,
        Mode: 'static + Send + Saturator<Sample = T>,
    > FreqAnalysis for Svf<T, Mode>
{
    #[replace_float_literals(Complex::from(T::cast_from(literal)))]
    fn h_z(
        &self,
        z: Complex<Self::Sample>,
    ) -> Matrix<Complex<Self::Sample>, <Self::Outputs as Enum>::Count, <Self::Inputs as Enum>::Count>
    {
        let omega_c = 2.0 * self.fc * self.w_step;
        let x0 = z + 1.0;
        let x1 = x0.powi(2) * omega_c.powi(2);
        let x2 = z - 1.0;
        let x3 = x2.powi(2) * 4.0;
        let x4 = x0 * x2 * omega_c;
        let x5 = (-x4 * 4.0 * self.r + x1 + x3).re.recip();
        NumericArray::from([NumericArray::from([x1 * x5, -x4 * x5 * 2.0, x3 * x5])])
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
    pub fn mix_coefficients<T: ops::Neg<Output = T> + ops::Sub<Output = T> + CastFrom<f64>>(
        &self,
        amp: T,
    ) -> [T; 4] {
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
