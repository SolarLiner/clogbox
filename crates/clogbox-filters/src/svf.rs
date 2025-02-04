//! Implementation of various blocks of DSP code from the VA Filter Design book.
//!
//! Downloaded from <https://www.discodsp.net/VAFilterDesign_2.1.2.pdf>
//! All references in this module, unless specified otherwise, are taken from this book.

use crate::{Linear, Saturator};
use az::{Cast, CastFrom};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{count, Enum, Mono};
use clogbox_params::smoothers::{ExpSmoother, Smoother};
use clogbox_schedule::module::{ExecutionContext, ProcessStatus, RawModule, SocketCount, SocketType, Sockets};
use clogbox_schedule::storage::SharedStorage;
use num_traits::{Float, FloatConst, Num, Zero};
use numeric_literals::replace_float_literals;
use std::marker::PhantomData;
use std::ops;

/// Parameter type for the SVF filter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Enum)]
pub enum SvfParams<SatParams> {
    /// Cutoff frequency (Hz)
    Cutoff,
    /// Resonance
    Resonance,
    /// Saturator parameter
    Saturator(SatParams),
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

impl<Mode: 'static + Send + Saturator<Sample: 'static + Send + Cast<f64> + CastFrom<f64> + Float>> RawModule
    for Svf<Mode>
where
    SvfParams<Mode::Params>: Enum,
{
    type Scalar = Mode::Sample;

    fn sockets(&self) -> Sockets {
        Sockets {
            inputs: SocketCount::new(|t| match t {
                SocketType::Audio => 2,
                SocketType::Param => count::<SvfParams<Mode::Params>>(),
                SocketType::Note => 0,
            }),
            outputs: SocketCount::new(|t| match t {
                SocketType::Audio => count::<SvfOutput>(),
                SocketType::Param => 0,
                SocketType::Note => 0,
            }),
        }
    }

    fn process(&mut self, ctx: &ExecutionContext<Self::Scalar>) -> ProcessStatus {
        let input = ctx.audio_storage.get(0);
        let mut buf_lp = ctx.audio_storage.get_mut(0);
        let mut buf_bp = ctx.audio_storage.get_mut(1);
        let mut buf_hp = ctx.audio_storage.get_mut(2);

        let params: EnumMapArray<_, _> =
            EnumMapArray::new(|p: SvfParams<Mode::Params>| ctx.param_storage.get(p.to_usize()));

        for i in 0..ctx.stream_data.buffer_size {
            params
                .iter()
                .filter_map(|(p, buf)| buf.event_at(i).copied().map(|ev| (p, ev)))
                .for_each(|(p, value)| {
                    self.set_param(p, value);
                });
            let (lp, bp, hp) = self.next_sample(input[i]);
            buf_lp[i] = lp;
            buf_bp[i] = bp;
            buf_hp[i] = hp;
        }
        ProcessStatus::Continue
    }
}

impl<Mode: 'static + Send + Saturator<Sample: 'static + Send + Cast<f64> + CastFrom<f64> + Float>> Svf<Mode> {
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

    pub fn set_param(&mut self, param: SvfParams<Mode::Params>, value: f32) {
        match param {
            SvfParams::Cutoff => self.set_cutoff(Mode::Sample::cast_from(value as _)),
            SvfParams::Resonance => self.set_resonance(Mode::Sample::cast_from(value as _)),
            SvfParams::Saturator(s) => self.saturator.set_param(s, value),
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
    __sample: PhantomData<T>,
}

impl<T: 'static + Copy + Send + Zero + Num + ops::Neg<Output = T> + CastFrom<f64>> RawModule for SvfMixer<T>
where
    ExpSmoother<T>: Smoother<T>,
{
    type Scalar = T;

    fn sockets(&self) -> Sockets {
        Sockets {
            inputs: SocketCount::new(|t| match t {
                SocketType::Audio => 1,
                SocketType::Param => count::<SvfMixerParams>(),
                SocketType::Note => 0,
            }),
            outputs: SocketCount::new(|t| match t {
                SocketType::Audio => 1,
                SocketType::Param => 0,
                SocketType::Note => 0,
            }),
        }
    }

    fn process(&mut self, context: &ExecutionContext<T>) -> ProcessStatus {
        use SvfMixerInput::*;
        use SvfMixerParams::*;

        let inputs: EnumMapArray<_, _> = EnumMapArray::new(|p: SvfMixerInput| context.audio_storage.get(p.to_usize()));
        let mut output = context.audio_storage.get_mut(0);
        let params: EnumMapArray<_, _> = EnumMapArray::new(|p: SvfMixerParams| context.param_storage.get(p.to_usize()));

        for i in 0..context.stream_data.buffer_size {
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
        ProcessStatus::Continue
    }
}
