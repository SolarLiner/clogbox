use az::Cast;
use crate::r#enum::Collection;
use num_traits::Float;

pub trait Interpolation<T> {
    fn interpolate(&self, values: &impl Collection<Item = T>, index: T) -> T;
}

pub struct Linear;

impl<T: Copy + Float + Cast<usize>> Interpolation<T> for Linear {
    fn interpolate(&self, values: &impl Collection<Item = T>, index: T) -> T {
        let i = index.floor().cast();
        let j = T::cast(index + T::one());
        let a = values[i];
        let b = values[j];
        a + (b - a) * index.fract()
    }
}
