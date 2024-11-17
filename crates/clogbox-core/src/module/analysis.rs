//! This module provides functionality related to frequency analysis of audio modules.
//! It defines types and traits used for performing analysis on audio signals, including
//! matrix types and operations, frequency conversion functions, and specialized traits
//! for frequency analysis of modules.
//!
//! The `Matrix` type alias represents a nested numeric array used extensively in frequency
//! analysis to store complex data structures. The `freq_to_z` function converts a given
//! frequency to a point on the complex unit circle, facilitating analysis in the Z-plane.
//!
//! The core trait `FreqAnalysis` extends the basic `Module` trait to include methods
//! specific to computing the frequency response of a module. This allows complex
//! frequency response calculations to be encapsulated within module implementations.
//!
//! # Example
//!
//! ```rust
//! use std::marker::PhantomData;
//! use std::ops;
//! use std::sync::Arc;
//! use az::CastFrom;
//! use num_complex::Complex;
//! use num_traits::Num;
//! use numeric_array::NumericArray;
//! use typenum::{Unsigned, U1};
//! use clogbox_core::math::dsp::freq_to_z;
//! use clogbox_core::module::analysis::{FreqAnalysis, Matrix};
//!
//! use clogbox_core::module::{BufferStorage, Module, ModuleContext, ProcessStatus, StreamData};
//! use clogbox_core::param::{Params, EMPTY_PARAMS};
//! use clogbox_core::r#enum::{enum_iter, Empty, Enum, Sequential};
//! use clogbox_core::r#enum::enum_map::EnumMapArray;
//!
//! struct Inverter<T, In>(PhantomData<(T, In)>);
//!
//!
//! impl<T, In> Default for Inverter <T, In>  {
//!     fn default() -> Self {
//!         Self(PhantomData)
//!     }
//! }
//!
//! impl<T: 'static + Send + Copy + Num + ops::Neg<Output=T>, In: 'static + Send + Enum> Module for Inverter<T, In> {
//!     type Sample = T;
//!     type Inputs = In;
//!     type Outputs = In;
//!     type Params = Empty;
//!
//!     fn get_params(&self) -> Arc<impl '_ + Params<Params=Self::Params>> {
//!         Arc::new(EMPTY_PARAMS)
//!     }
//!
//!     fn supports_stream(&self, data: StreamData) -> bool {
//!         true
//!     }
//!
//!     fn latency(&self, input_latency: EnumMapArray<Self::Inputs, f64>) -> EnumMapArray<Self::Outputs, f64> {
//!         input_latency
//!     }
//!
//!     fn process<S: BufferStorage<Sample=Self::Sample, Input=Self::Inputs, Output=Self::Outputs>>(&mut self, context: &mut ModuleContext<S>) -> ProcessStatus {
//!         for inp in enum_iter::<In>() {
//!             let (inp_buf, out_buf) = context.get_input_output_pair(inp, inp);
//!             for (o, i) in out_buf.iter_mut().zip(inp_buf.iter()) {
//!                 *o = -*i;
//!             }
//!         }
//!         ProcessStatus::Running
//!     }
//! }
//!
//! impl<T: 'static + Send + Copy + Num + ops::Neg<Output=T> + CastFrom<f64>, In: 'static + Send + Enum> FreqAnalysis for Inverter<T, In> {
//!     fn h_z(&self, z: Complex<Self::Sample>) -> Matrix<Complex<Self::Sample>, In::Count, In::Count> {
//!         NumericArray::from_iter((0..In::Count::USIZE).map(|_| NumericArray::from_iter(std::iter::repeat(Complex::from(T::cast_from(-1.0))).take(In::Count::USIZE))))
//!     }
//! }
//!
//! let module = Inverter::<f32, Sequential<U1>>::default();
//! let z_plane_point = freq_to_z(44100.0, 1000.0);
//! let response = module.freq_response(44100., 1000.);
//! assert_eq!(response[0][0], Complex::from(-1.0));
//! ```
use crate::math::dsp::freq_to_z;
use crate::module::Module;
use crate::r#enum::Enum;
use num_complex::Complex;
use num_traits::{Float, FloatConst};
use numeric_array::{ArrayLength, NumericArray};

/// A type alias for a matrix represented as a nested `NumericArray`.
/// `T` is the type of the elements, `R` is the number of rows, and `C` is the number of columns.
pub type Matrix<T, R, C> = NumericArray<NumericArray<T, R>, C>;

/// A trait for frequency analysis modules, which are a specialized type of `Module`.
///
/// This trait extends the `Module` trait with functionality specific to frequency analysis.
#[allow(clippy::type_complexity)]
pub trait FreqAnalysis: Module
where
    <Self::Inputs as Enum>::Count: ArrayLength,
    <Self::Outputs as Enum>::Count: ArrayLength,
{
    /// Computes the frequency response of the module at a given point `z` on the complex plane.
    ///
    /// # Arguments
    ///
    /// * `z` - A point on the complex plane representing the frequency to analyze.
    ///
    /// # Returns
    ///
    /// A matrix representing the frequency response of the module at the given point.
    fn h_z(
        &self,
        z: Complex<Self::Sample>,
    ) -> Matrix<Complex<Self::Sample>, <Self::Outputs as Enum>::Count, <Self::Inputs as Enum>::Count>;

    /// Computes the frequency response of the module at a given frequency `freq`.
    ///
    /// This method converts the frequency to a point `z` on the complex unit circle and then
    /// computes the frequency response using the `h_z` method.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - The sampling rate of the signal.
    /// * `freq` - The frequency to analyze.
    ///
    /// # Returns
    ///
    /// A matrix representing the frequency response of the module at the given frequency.
    #[inline]
    fn freq_response(
        &self,
        sample_rate: Self::Sample,
        freq: Self::Sample,
    ) -> Matrix<Complex<Self::Sample>, <Self::Outputs as Enum>::Count, <Self::Inputs as Enum>::Count>
    where
        Self::Sample: Float + FloatConst,
    {
        self.h_z(freq_to_z(sample_rate, freq))
    }
}
