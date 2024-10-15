use num_complex::Complex;
use num_traits::{Float, FloatConst};

#[inline]
pub fn freq_to_z<T: Float + FloatConst>(samplerate: T, f: T) -> Complex<T>
{
    let jw = Complex::new(T::zero(), T::TAU() * f / samplerate);
    jw.exp()
}
