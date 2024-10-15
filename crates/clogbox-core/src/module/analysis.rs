use std::ops;
use num_complex::Complex;
use num_traits::{Float, FloatConst};
use numeric_array::ArrayLength;
use numeric_array::generic_array::GenericArray;
use crate::math::dsp::freq_to_z;
use crate::module::Module;
use crate::r#enum::Enum;

pub struct Matrix<T, R: ArrayLength, C: ArrayLength> {
    pub data: GenericArray<GenericArray<T, R>, C>,
}

impl<T, R: ArrayLength, C: ArrayLength> ops::Index<usize> for Matrix<T, R, C> {
    type Output = GenericArray<T, R>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl<T, R: ArrayLength, C: ArrayLength> ops::IndexMut<usize> for Matrix<T, R, C> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[index]
    }
}

impl<T, R: ArrayLength, C: ArrayLength> ops::Index<(usize, usize)> for Matrix<T, R, C> {
    type Output = T;

    fn index(&self, (r, c): (usize, usize)) -> &Self::Output {
        &self.data[c][r]
    }
}

impl<T, R: ArrayLength, C: ArrayLength> ops::IndexMut<(usize, usize)> for Matrix<T, R, C> {
    fn index_mut(&mut self, (r, c): (usize, usize)) -> &mut Self::Output {
        &mut self.data[c][r]
    }
}

impl<T, R: ArrayLength, C: ArrayLength> ops::Index<[usize; 2]> for Matrix<T, R, C> {
    type Output = T;

    fn index(&self, [r, c]: [usize; 2]) -> &Self::Output {
        &self.data[c][r]
    }
}

impl<T, R: ArrayLength, C: ArrayLength> ops::IndexMut<[usize; 2]> for Matrix<T, R, C> {
    fn index_mut(&mut self, [r, c]: [usize; 2]) -> &mut Self::Output {
        &mut self.data[c][r]
    }
}

pub trait FreqAnalysis: Module where <Self::Inputs as Enum>::Count: ArrayLength, <Self::Outputs as Enum>::Count: ArrayLength {
    fn h_z(&self, z: Complex<Self::Sample>) -> Matrix<Self::Sample, <Self::Outputs as Enum>::Count, <Self::Inputs as Enum>::Count>;
    
    #[inline]
    fn freq_response(&self, samplerate: Self::Sample, freq: Self::Sample) -> Matrix<Self::Sample, <Self::Outputs as Enum>::Count, <Self::Inputs as Enum>::Count> where Self::Sample: Float + FloatConst {
        self.h_z(freq_to_z(samplerate, freq))
    }
}