//! Implementation of various blocks of DSP code from the VA Filter Design book.
//!
//! Downloaded from <https://www.discodsp.net/VAFilterDesign_2.1.2.pdf>
//! All references in this module, unless specified otherwise, are taken from this book.

use crate::svf::SvfOutput::{Bandpass, Highpass};
use crate::{Linear, Saturator};
use az::{Cast, CastFrom};
use clogbox_core::module::analysis::{FreqAnalysis, Matrix};
use clogbox_core::module::sample::{SampleContext, SampleContextImpl, SampleModule};
use clogbox_core::module::utilitarian::SummingMatrix;
use clogbox_core::module::{ProcessStatus, StreamData};
use clogbox_core::param::enum_range;
use clogbox_core::param::events::ParamEventsExt;
use clogbox_core::param::Params;
use clogbox_core::r#enum::enum_map::{EnumMapArray, EnumMapMut};
use clogbox_core::r#enum::{Empty, Enum, Mono};
use clogbox_derive::{Enum, Params};
use num_complex::Complex;
use num_traits::{Float, FloatConst, Num, Zero};
use numeric_array::NumericArray;
use numeric_literals::replace_float_literals;
use std::marker::PhantomData;
use std::ops;
use typenum::Unsigned;

/// Parameter type for the SVF filter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Enum, Params)]
pub enum SvfParams<SatParams> {
    /// Cutoff frequency (Hz)
    Cutoff,
    /// Resonance
    Resonance,
    Saturator(SatParams),
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
pub struct Svf<Mode: Saturator> {
    s: [Mode::Sample; 2],
    r: Mode::Sample,
    fc: Mode::Sample,
    g: Mode::Sample,
    g1: Mode::Sample,
    d: Mode::Sample,
    w_step: Mode::Sample,
    sample_rate: Mode::Sample,
    saturator: Mode,
}

impl<Mode: Saturator> Svf<Mode> {
    /// Apply these new saturators to this SVF instance, returning a new instance of it.
    pub fn set_saturator(&mut self, saturator: Mode) {
        self.saturator = saturator;
    }

    /// Replace the saturators in this SVF instance with the provided values.
    pub fn with_saturator<S2: Saturator<Sample = Mode::Sample>>(self, saturator: S2) -> Svf<S2> {
        let Self {
            s,
            r,
            fc,
            g,
            g1,
            d,
            w_step,
            sample_rate,
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
        }
    }
}

impl<T: Copy + Float + FloatConst + CastFrom<f64> + Num> Svf<Linear<T>> {
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
        };
        this.update_coefficients();
        this
    }
}
impl<Sat: Saturator<Sample: Cast<f64> + CastFrom<f64> + Float>> Svf<Sat> {
    /// Set the new filter cutoff frequency (in Hz).
    pub fn set_cutoff(&mut self, freq: Sat::Sample) {
        self.set_cutoff_no_update(freq);
        self.update_coefficients();
    }

    /// Sets the cutoff frequency (in Hz) without triggering a recomputation of internal parameters.
    /// It's required to call [`Self::update_coefficients`] after a call to this, and before
    /// processing the next sample.
    pub fn set_cutoff_no_update(&mut self, freq: Sat::Sample) {
        self.fc = freq;
    }

    /// Set the resonance amount (in 0..1 for stable filters, otherwise use bounded nonlinearities).
    pub fn set_resonance(&mut self, r: Sat::Sample) {
        self.set_resonance_no_update(r);
        self.update_coefficients();
    }

    /// Sets the cutoff frequency (in Hz) without triggering a recomputation of internal parameters.
    /// It's required to call [`Self::update_coefficients`] after a call to this, and before
    /// processing the next sample.
    #[replace_float_literals(Sat::Sample::cast_from(literal))]
    pub fn set_resonance_no_update(&mut self, r: Sat::Sample) {
        let r = 1. - r;
        self.r = 2. * r;
    }
}

impl<Sat: Saturator<Sample: Float + CastFrom<f64>>> Svf<Sat> {
    /// Recompute the internal parameters from the cutoff and resonance parameters.
    #[profiling::function]
    #[replace_float_literals(Sat::Sample::cast_from(literal))]
    pub fn update_coefficients(&mut self) {
        self.g = self.w_step * self.fc;
        self.g1 = 2. * self.r + self.g;
        self.d = (1. + 2. * self.r * self.g + self.g * self.g).recip();
    }
}

impl<
        Mode: 'static + Send + Saturator<Sample: 'static + Send + Cast<f64> + CastFrom<f64> + Float>,
    > SampleModule for Svf<Mode>
where
    SvfParams<Mode::Params>: Params,
{
    type Sample = Mode::Sample;
    type Inputs = SvfInput;
    type Outputs = SvfOutput;
    type Params = SvfParams<Mode::Params>;

    fn reset(&mut self) {
        self.s.fill(Self::Sample::zero())
    }

    fn latency(&self) -> f64 {
        2.0
    }

    #[replace_float_literals(Mode::Sample::cast_from(literal))]
    fn process_sample(&mut self, mut context: SampleContext<Self>) -> ProcessStatus {
        use SvfInput::*;
        use SvfOutput::*;
        let x = context.inputs[AudioInput];

        let (hp, bp, lp) = self.next_sample(
            EnumMapArray::new(|p| context.params[SvfParams::Saturator(p)]),
            x,
        );
        context.outputs[Lowpass] = lp;
        context.outputs[Bandpass] = bp;
        context.outputs[Highpass] = hp;
        ProcessStatus::Running
    }
}

impl<
        Mode: 'static + Send + Saturator<Sample: 'static + Send + Cast<f64> + CastFrom<f64> + Float>,
    > Svf<Mode>
{
    /// Process the next sample of this filter, with the given input sample.
    ///
    /// The output samples are `(LP, BP, HP)`
    #[replace_float_literals(Mode::Sample::cast_from(literal))]
    pub fn next_sample(
        &mut self,
        params: EnumMapArray<Mode::Params, f32>,
        x: Mode::Sample,
    ) -> (Mode::Sample, Mode::Sample, Mode::Sample) {
        let [s1, s2] = self.s;

        let bpp = self.saturator.saturate(params, s1);
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
        (hp, bp, lp)
    }
}

impl<
        Mode: 'static
            + Send
            + Saturator<Sample: 'static + Send + Copy + Zero + CastFrom<f64> + Cast<f64> + Float>,
    > FreqAnalysis for Svf<Mode>
where
    SvfParams<Mode::Params>: Params,
{
    #[replace_float_literals(Complex::from(Mode::Sample::cast_from(literal)))]
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

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum, Params)]
pub enum SvfMixerParams {
    #[display = "Filter Type"]
    #[param(range = "enum_range::<FilterType>()")]
    FilterType,
    #[param(range = "-1.0..=1.0")]
    Amplitude,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Enum)]
pub enum SvfMixerInput {
    AudioInput,
    SvfOutput(SvfOutput),
}

pub struct SvfMixer<T> {
    filter_type: FilterType,
    __sample: PhantomData<T>,
}

impl<T: 'static + Copy + Send + Zero + Num + std::ops::Neg<Output = T> + CastFrom<f64>> SampleModule
    for SvfMixer<T>
{
    type Sample = T;
    type Inputs = SvfMixerInput;
    type Outputs = Mono;
    type Params = SvfMixerParams;

    fn process_sample(&mut self, mut context: SampleContext<Self>) -> ProcessStatus {
        use self::Mono::*;
        use self::SvfOutput::*;
        use SvfMixerInput::*;
        use SvfMixerParams::*;

        let filter_type = context.params[FilterType].get_enum::<self::FilterType>(0);
        let inputs = [
            AudioInput,
            SvfOutput(Lowpass),
            SvfOutput(Bandpass),
            SvfOutput(Highpass),
        ]
        .map(|input| context.inputs[input]);
        let output = inputs
            .into_iter()
            .zip(filter_type.mix_coefficients(T::cast_from(context.params[Amplitude] as _)))
            .map(|(a, b)| a * b)
            .reduce(ops::Add::add)
            .unwrap_or_else(T::zero);
        context.outputs[Mono] = output;
        ProcessStatus::Running
    }
}
