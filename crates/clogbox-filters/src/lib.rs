#![warn(missing_docs)]
//! Implementation of non-linear filters.
//!
//! This module provides a number of non-linear filters that can be used to modify the
//! amplitude of audio signals.

use az::CastFrom;
use clogbox_enum::enum_map::{EnumMapArray, EnumMapRef};
use clogbox_enum::{Empty, Enum, Mono};
use clogbox_module::context::StreamContext;
use clogbox_module::sample::{SampleModule, SampleProcessResult};
use clogbox_module::{PrepareResult, Samplerate};
use num_traits::{Float, FloatConst, Num};
use std::num::NonZeroU32;

pub mod biquad;
pub mod saturators;
pub mod svf;

pub use saturators::Saturator;

/// Type of parameters in a [`Multimode`] module.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Enum)]
pub enum MultimodeParams {
    /// Cutoff parameter
    Cutoff,
}

/// 1-pole multi-mode filter producing a low-pass output.
///
/// High-pass can be generated with $y_{hp} = x - y$, and all-pass with $y - y_{hp} = 2 y - x$.
pub struct Multimode<T> {
    s: T,
    cutoff: T,
    wstep: T,
    g: T,
    g1: T,
}

impl<T: Float + FloatConst> Multimode<T> {
    /// Create a new [`Multimode`] filter running at the given sample rate with the given cutoff.
    ///
    /// # Arguments
    ///
    /// * `samplerate`: Sample rate of the audio signal (Hz)
    /// * `cutoff`: Cutoff frequency of the filter (Hz)
    pub fn new(samplerate: T, cutoff: T) -> Self {
        let wstep = T::TAU() / samplerate;
        let g = wstep * cutoff;
        let g1 = g / (T::one() + g);
        Self {
            s: T::zero(),
            cutoff,
            wstep,
            g,
            g1,
        }
    }

    /// Set the sample rate of the signal running through this filter (in Hz).
    pub fn set_samplerate(&mut self, samplerate: T) {
        self.wstep = T::TAU() / samplerate;
        self.g = self.wstep * self.cutoff;
    }
}

impl<T: Float> Multimode<T> {
    /// Set the cutoff frequency of the filter (in Hz).
    pub fn set_cutoff(&mut self, cutoff: T) {
        if self.cutoff == cutoff {
            return;
        }
        self.cutoff = cutoff;
        self.g = self.wstep * self.cutoff;
        self.g1 = self.g / (T::one() + self.g);
    }

    /// Set a parameter to this module
    pub fn set_param(&mut self, _: MultimodeParams, value: T) {
        self.set_cutoff(value);
    }

    /// Compute the output sample of an audio signal being filtered through this Multimode filter.
    #[inline]
    pub fn next_sample(&mut self, input: T) -> T {
        let v = (input - self.s) * self.g1;
        let y = v + self.s;
        self.s = y + v;
        y
    }
}

impl<T: CastFrom<f64> + Float + FloatConst> SampleModule for Multimode<T> {
    type Sample = T;
    type AudioIn = Mono;
    type AudioOut = Mono;
    type Params = MultimodeParams;

    fn prepare(&mut self, sample_rate: Samplerate) -> PrepareResult {
        self.set_samplerate(T::cast_from(sample_rate.value()));
        PrepareResult { latency: 0.0 }
    }

    fn process(
        &mut self,
        _stream_context: &StreamContext,
        inputs: EnumMapArray<Self::AudioIn, Self::Sample>,
        params: EnumMapRef<Self::Params, f32>,
    ) -> SampleProcessResult<Self::AudioOut, Self::Sample> {
        self.set_cutoff(T::cast_from(params[MultimodeParams::Cutoff] as _));
        let out = self.next_sample(inputs[Mono]);
        let output = EnumMapArray::from_std_array([out]);
        SampleProcessResult {
            tail: NonZeroU32::new((T::cast_from(5.0) / self.cutoff).to_u32().unwrap_or(0)),
            output,
        }
    }
}

/// DC Blocker filter running a high-pass filter pre-configured at 7 Hz.
pub struct DcBlocker<T> {
    mm: Multimode<T>,
}

impl<T: CastFrom<f64> + Float + FloatConst> DcBlocker<T> {
    /// Create a new DC blocker module
    pub fn new(samplerate: T) -> Self {
        Self {
            mm: Multimode::new(samplerate, T::cast_from(7.0)),
        }
    }

    /// Compute a single sample of the audio signal running through this module.
    pub fn next_sample(&mut self, input: T) -> T {
        let lp = self.mm.next_sample(input);
        input - lp
    }
}

impl<T: CastFrom<f64> + Float + FloatConst> SampleModule for DcBlocker<T> {
    type Sample = T;
    type AudioIn = Mono;
    type AudioOut = Mono;
    type Params = Empty;

    fn prepare(&mut self, sample_rate: Samplerate) -> PrepareResult {
        self.mm.set_samplerate(T::cast_from(sample_rate.value()));
        PrepareResult { latency: 0.0 }
    }

    fn process(
        &mut self,
        _stream_context: &StreamContext,
        inputs: EnumMapArray<Self::AudioIn, Self::Sample>,
        _params: EnumMapRef<Self::Params, f32>,
    ) -> SampleProcessResult<Self::AudioOut, Self::Sample> {
        let out = self.next_sample(inputs[Mono]);
        let output = EnumMapArray::from_std_array([out]);
        SampleProcessResult { tail: None, output }
    }
}
