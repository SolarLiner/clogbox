//! Implementation of various blocks of DSP code from the VA Filter Design book.
//!
//! Downloaded from <https://www.discodsp.net/VAFilterDesign_2.1.2.pdf>
//! All references in this module, unless specified otherwise, are taken from this book.

use crate::{Linear, Saturator};
use az::{Cast, CastFrom};
use clogbox_core::graph::context::GraphContext;
use clogbox_core::graph::module::{Module, ModuleError, ProcessStatus};
use clogbox_core::graph::slots::Slots;
use clogbox_core::graph::SlotType;
use clogbox_params::smoothers::{ExpSmoother, Smoother};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{Enum, Mono};
use num_traits::{Float, FloatConst, Num, Zero};
use numeric_literals::replace_float_literals;
use std::marker::PhantomData;
use std::ops;

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
pub enum SvfInput<SatParams> {
    /// Audio input for the SVF.
    AudioInput,
    // SVF Parameters repeated because of a limitation of the enum derive macro
    /// Cutoff frequency (Hz)
    Cutoff,
    /// Resonance
    Resonance,
    /// Inner saturator parameters
    SaturatorParams(SatParams),
}

impl<SatParams> From<SvfParams> for SvfInput<SatParams> {
    fn from(value: SvfParams) -> Self {
        match value {
            SvfParams::Cutoff => Self::Cutoff,
            SvfParams::Resonance => Self::Resonance,
        }
    }
}

impl<SatParams> Slots for SvfInput<SatParams>
where
    Self: Enum,
{
    fn slot_type(&self) -> SlotType {
        match self {
            Self::AudioInput => SlotType::Audio,
            Self::SaturatorParams(_) => SlotType::Control,
            _ => SlotType::Control, // SVF Parameters
        }
    }
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

impl Slots for SvfOutput {
    fn slot_type(&self) -> SlotType {
        SlotType::Audio
    }
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
    > Module for Svf<Mode>
where
    SvfInput<Mode::Params>: Slots,
{
    type Sample = Mode::Sample;
    type Inputs = SvfInput<Mode::Params>;
    type Outputs = SvfOutput;

    fn process(&mut self, graph_context: GraphContext<Self>) -> Result<ProcessStatus, ModuleError> {
        let input = graph_context.get_audio_input(SvfInput::AudioInput)?;
        let mut buf_lp = graph_context.get_audio_output(SvfOutput::Lowpass)?;
        let mut buf_bp = graph_context.get_audio_output(SvfOutput::Bandpass)?;
        let mut buf_hp = graph_context.get_audio_output(SvfOutput::Highpass)?;
        let params: EnumMapArray<_, _> =
            EnumMapArray::new(|p: SvfParams| graph_context.get_control_input(p.into()))
                .transpose()?;
        let sat_params: EnumMapArray<_, _> =
            EnumMapArray::new(|p| graph_context.get_control_input(SvfInput::SaturatorParams(p)))
                .transpose()?;

        for i in 0..graph_context.stream_data().block_size {
            params
                .iter()
                .filter_map(|(p, buf)| buf.event_at(i).copied().map(|ev| (p, ev)))
                .for_each(|(p, value)| {
                    self.set_param(p, value);
                });
            sat_params
                .iter()
                .filter_map(|(p, buf)| buf.event_at(i).copied().map(|ev| (p, ev)))
                .for_each(|(p, value)| self.set_saturator_param(p, value));
            let (lp, bp, hp) = self.next_sample(input[i]);
            buf_lp[i] = lp;
            buf_bp[i] = bp;
            buf_hp[i] = hp;
        }
        Ok(ProcessStatus::Tail(2))
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
    pub fn next_sample(&mut self, x: Mode::Sample) -> (Mode::Sample, Mode::Sample, Mode::Sample) {
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
        (lp, bp, hp)
    }

    pub fn set_param(&mut self, param: SvfParams, value: f32) {
        match param {
            SvfParams::Cutoff => self.set_cutoff(Mode::Sample::cast_from(value as _)),
            SvfParams::Resonance => self.set_resonance(Mode::Sample::cast_from(value as _)),
        }
    }

    pub fn set_saturator_param(&mut self, param: Mode::Params, value: f32) {
        self.saturator.set_param(param, value);
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

impl Slots for SvfMixerInput {
    fn slot_type(&self) -> SlotType {
        match self {
            Self::AudioInput | Self::SvfOutput(_) => SlotType::Audio,
            Self::Params(_) => SlotType::Control,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SvfMixer<T> {
    filter_type: FilterType,
    amp: ExpSmoother<T>,
    coeffs: [ExpSmoother<T>; 4],
    __sample: PhantomData<T>,
}

impl<T: 'static + Copy + Send + Zero + Num + ops::Neg<Output = T> + CastFrom<f64>> Module
    for SvfMixer<T>
where
    ExpSmoother<T>: Smoother<T>,
{
    type Sample = T;
    type Inputs = SvfMixerInput;
    type Outputs = Mono;

    fn process(&mut self, context: GraphContext<Self>) -> Result<ProcessStatus, ModuleError> {
        use SvfMixerInput::*;
        use SvfMixerParams::*;

        let inputs: EnumMapArray<_, _> =
            EnumMapArray::new(|p| context.get_audio_input(p)).transpose()?;
        let mut output = context.get_audio_output(Mono)?;
        let params: EnumMapArray<_, _> =
            EnumMapArray::new(|p| context.get_control_input(Params(p))).transpose()?;

        for i in 0..context.stream_data().block_size {
            for (p, value) in params
                .iter()
                .filter_map(|(p, buf)| buf.event_at(i).copied().map(|ev| (p, ev)))
            {
                match p {
                    FilterType => {
                        self.filter_type = self::FilterType::from_usize(value as _);
                    }
                    Amplitude => {
                        self.amp.set_target(T::cast_from(value as _));
                    }
                }
            }

            let amp = self.amp.next_value();
            let k = self.filter_type.mix_coefficients(amp);
            for (smoother, v) in self.coeffs.iter_mut().zip(k) {
                smoother.set_target(v);
            }

            let k = self.coeffs.each_mut().map(|s| s.next_value());
            output[i] = inputs
                .values()
                .map(|buf| buf[i])
                .zip(k)
                .map(|(x, k)| x * k)
                .fold(T::zero(), ops::Add::add);
        }
        Ok(ProcessStatus::Running)
    }
}
