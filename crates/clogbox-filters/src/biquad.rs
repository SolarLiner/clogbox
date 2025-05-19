//! Biquad filter, implemented on top of the [`Svf`] filter in linear mode.
use crate::svf::{Svf, SvfOutput};
use az::CastFrom;
use clogbox_enum::enum_map::{EnumMap, EnumMapArray, EnumMapRef};
use clogbox_enum::{Empty, Enum, Mono};
use clogbox_module::context::StreamContext;
use clogbox_module::sample::{SampleModule, SampleProcessResult};
use clogbox_module::{PrepareResult, Samplerate};
use generic_array::GenericArray;
use num_traits::{Float, FloatConst};
use numeric_literals::replace_float_literals;

/// Compute the Biquad coefficients for a low-pass filter
///
/// # Arguments
///
/// * `cutoff`: Normalized (i.e., where samplerate == 1) cutoff frequency
/// * `q`: Resonance
#[replace_float_literals(T::cast_from(literal))]
pub fn lowpass<T: CastFrom<f64> + Float + FloatConst>(cutoff: T, q: T) -> ([T; 3], [T; 3]) {
    let w0 = T::TAU() * cutoff;
    let (sw0, cw0) = w0.sin_cos();
    let b1 = 1. - cw0;
    let b0 = b1 / 2.;
    let b2 = b0;

    let alpha = sw0 / (2. * q);
    let a0 = 1. + alpha;
    let a1 = -2. * cw0;
    let a2 = 1. - alpha;
    ([1.0, a1 / a0, a2 / a0], [b0, b1, b2].map(|b| b / a0))
}

/// Compute the Biquad coefficients for a high-pass filter
///
/// # Arguments
///
/// * `cutoff`: Normalized (i.e., where samplerate == 1) cutoff frequency
/// * `q`: Resonance
#[replace_float_literals(T::cast_from(literal))]
pub fn highpass<T: CastFrom<f64> + Float + FloatConst>(cutoff: T, q: T) -> ([T; 3], [T; 3]) {
    let w0 = T::TAU() * cutoff;
    let (sw0, cw0) = w0.sin_cos();
    let b1 = -(1. + cw0);
    let b0 = -b1 / 2.;
    let b2 = b0;

    let alpha = sw0 / (2. * q);
    let a0 = 1. + alpha;
    let a1 = -2. * cw0;
    let a2 = 1. - alpha;
    ([1.0, a1 / a0, a2 / a0], [b0, b1, b2].map(|b| b / a0))
}

/// Biquad filter based on mapping 2nd-order coefficients to an SVF and mixing the outputs
pub struct Biquad<T> {
    out_coeffs: EnumMapArray<SvfOutput, T>,
    svf: Svf<T>,
    samplerate: T,
    a: [T; 3],
    b: [T; 3],
}

impl<T: CastFrom<f64> + Float + FloatConst> Biquad<T> {
    /// Create a Biquad filter from the provided `a` and `b` coefficients of a generalized 2nd-order filter.
    ///
    /// # Arguments
    ///
    /// * `samplerate`: Sample rate the filter is going to run at
    /// * `a`: Denominator (zeros) coefficients
    /// * `b`: Numerator (poles) coefficients
    ///
    /// # Examples
    ///
    /// ```
    /// use clogbox_filters::biquad;
    /// use biquad::Biquad;
    ///
    /// let (a, b) = biquad::lowpass(0.5, 0.707);
    /// let  biquad = Biquad::new(44100.0, a, b);
    /// ```
    #[replace_float_literals(T::cast_from(literal))]
    pub fn new(samplerate: T, a: [T; 3], b: [T; 3]) -> Self {
        let (wc, q, out_coeffs) = Self::compute_coeffs(samplerate, a, b);
        let svf = Svf::new(samplerate, wc, q);
        Self {
            out_coeffs,
            svf,
            samplerate,
            a,
            b,
        }
    }
}

impl<T: CastFrom<f64> + Float> Biquad<T> {
    /// Set this Biquad filter to the provided coefficients.
    ///
    /// # Arguments
    ///
    /// * `a`: Denominator (zeros) coefficients
    /// * `b`: Numerator (poles) coefficients
    ///
    /// # Examples
    ///
    /// ```
    /// use clogbox_filters::biquad;
    /// use biquad::Biquad;
    ///
    /// let mut filter = Biquad::new(44100.0, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    /// let (a, b) = biquad::lowpass(0.5, 0.707);
    /// filter.set_coefficients(a, b);
    /// ```
    pub fn set_coefficients(&mut self, a: [T; 3], b: [T; 3]) {
        let (wc, q, out_coeffs) = Self::compute_coeffs(self.samplerate, a, b);
        self.svf.set_cutoff_no_update(wc);
        self.svf.set_resonance(q);
        self.a = a;
        self.b = b;
        self.out_coeffs = out_coeffs;
    }

    /// Sets the sample rate of the audio signal going through this module.
    pub fn set_samplerate(&mut self, samplerate: T) {
        if self.samplerate == samplerate {
            return;
        }
        self.samplerate = samplerate;
        self.set_coefficients(self.a, self.b);
    }

    #[replace_float_literals(T::cast_from(literal))]
    fn compute_coeffs(
        samplerate: T,
        a: [T; 3],
        b: [T; 3],
    ) -> (T, T, EnumMap<SvfOutput, GenericArray<T, <SvfOutput as Enum>::Count>>) {
        let wc = a[0].sqrt();
        let r = a[1] / wc;
        // r = 2 * (1 - q) <=> q = 1 - r/2
        let q = 1.0 - r / 2.0;
        let out_coeffs = EnumMapArray::new(|e| match e {
            SvfOutput::Lowpass => b[0] / (wc * wc),
            SvfOutput::Bandpass => b[1] / wc,
            SvfOutput::Highpass => b[2],
        });
        (samplerate * wc, q, out_coeffs)
    }
}

impl<T: Float + CastFrom<f64>> Biquad<T> {
    /// Process a single sample of audio
    ///
    /// # Arguments
    ///
    /// * `input`: Input audio sample
    pub fn next_sample(&mut self, input: T) -> T {
        let svf = self.svf.next_sample(input);
        svf.into_iter()
            .map(|(e, f)| self.out_coeffs[e] * f)
            .reduce(|a, b| a + b)
            .unwrap_or_else(T::zero)
    }
}

impl<T: CastFrom<f64> + Float + FloatConst> SampleModule for Biquad<T> {
    type Sample = T;
    type AudioIn = Mono;
    type AudioOut = Mono;
    type Params = Empty;

    fn prepare(&mut self, sample_rate: Samplerate) -> PrepareResult {
        self.set_samplerate(T::cast_from(sample_rate.value()));
        PrepareResult { latency: 0.0 }
    }

    fn process(
        &mut self,
        _stream_context: &StreamContext,
        inputs: EnumMapArray<Self::AudioIn, Self::Sample>,
        _params: EnumMapRef<Self::Params, f32>,
    ) -> SampleProcessResult<Self::AudioOut, Self::Sample> {
        let out = self.next_sample(inputs[Mono]);
        SampleProcessResult {
            tail: None,
            output: EnumMapArray::from_std_array([out]),
        }
    }
}
