use az::CastFrom;
use num_traits::Float;
use numeric_literals::replace_float_literals;

pub mod dsp;
pub mod interpolation;
pub mod recip;
pub mod root_eq;

#[replace_float_literals(T::cast_from(literal))]
pub fn db_to_linear<T: Float + CastFrom<f64>>(db: T) -> T {
    10.0_f64.powf(db / 20.0)
}

#[replace_float_literals(T::cast_from(literal))]
pub fn linear_to_db<T: Float + CastFrom<f64>>(linear: T) -> T {
    20.0 * linear.log10()
}
