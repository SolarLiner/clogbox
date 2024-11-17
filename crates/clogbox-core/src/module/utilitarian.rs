//! This module provides the core functionalities and structures for handling various
//! audio processing components. It includes definitions for processing statuses,
//! stream metadata, and configuration, as well as implementations of different
//! processing units.
use crate::math::recip::Recip;
use crate::module::{BufferStorage, Module, ModuleContext, ProcessStatus, StreamData};
use crate::param::{Value, Params};
use crate::r#enum::enum_map::{EnumMap, EnumMapArray, EnumMapBox};
use crate::r#enum::{enum_iter, CartesianProduct, Enum};
use az::{Cast, CastFrom};
use num_traits::{Num, NumAssign, Zero};
use numeric_array::ArrayLength;
use std::marker::PhantomData;
use std::ops;
use std::sync::Arc;
use typenum::Unsigned;
use crate::param::smoother::{LinearSmoother, Smoother};

/// A matrix that sums the inputs given a matrix of input:output coefficients.
///
/// `SummingMatrix` uses an `EnumMapBox` to hold parameters of type `ParamCurve`
/// for each combination of `In` and `Out` types represented by `CartesianProduct`.
///
/// # Type Parameters
/// * `T` - The type used for the sample data.
/// * `In` - The type representing the input parameters.
/// * `Out` - The type representing the output parameters.
#[derive(Debug, Clone)]
pub struct SummingMatrix<T, In, Out> {
    params: Arc<EnumMapBox<CartesianProduct<In, Out>, Value>>,
    smoothers: Option<EnumMapBox<CartesianProduct<In, Out>, LinearSmoother>>,
    __sample: PhantomData<fn(T) -> T>,
}

impl<T, In, Out> SummingMatrix<T, In, Out> {
    const PARAMS_MAX_TIMESTAMPS: usize = 64;
}

impl<T, In: Enum, Out: Enum> SummingMatrix<T, In, Out>
where
    T: Copy,
    In::Count: ops::Mul<Out::Count>,
    <In::Count as ops::Mul<Out::Count>>::Output: Unsigned + ArrayLength,
{
    /// Creates a new `SummingMatrix` with the specified parameters.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - The sample rate (in Hz) used to interpret the timestamps.
    /// * `max_timestamps` - The maximum number of timestamps each `ParamCurve` can store.
    /// * `initial_values` - An `EnumMapBox` providing the initial values for the `ParamCurve` for each combination of `In` and `Out`.
    pub fn new(
        sample_rate: impl Into<Recip<f32>>,
        initial_values: EnumMapBox<CartesianProduct<In, Out>, f32>,
        smoothing_time: impl Into<Option<f32>>,
    ) -> Self {
        let sample_rate = sample_rate.into();
        Self {
            params: Arc::new(EnumMap::new(|k| Value::new(initial_values[k]))),
            smoothers: smoothing_time
                .into()
                .map(|time| EnumMap::new(|k| LinearSmoother::new(0., 0., time, sample_rate))),
            __sample: PhantomData,
        }
    }
}

impl<
        T: 'static + Copy + Send + NumAssign + Num + Zero + CastFrom<f32>,
        In: 'static + Send + Sync + Enum,
        Out: 'static + Send + Sync + Enum,
    > Module for SummingMatrix<T, In, Out>
where
    In::Count: ops::Mul<Out::Count, Output: Unsigned + ArrayLength>,
{
    type Sample = T;
    type Inputs = In;
    type Outputs = Out;
    type Params = CartesianProduct<In, Out>;

    fn get_params(&self) -> Arc<impl Params<Params= Self::Params>> {
        self.params.clone()
    }

    fn supports_stream(&self, _: StreamData) -> bool {
        true
    }

    fn latency(
        &self,
        input_latencies: EnumMapArray<Self::Inputs, f64>,
    ) -> EnumMapArray<Self::Outputs, f64> {
        EnumMapArray::new(|out| {
            input_latencies
                .iter()
                .map(|(k, &v)| v * self.params[CartesianProduct(k, out)].get_value_normalized() as f64)
                .sum()
        })
    }

    #[inline]
    #[profiling::function]
    fn process<
        S: BufferStorage<Sample = Self::Sample, Input = Self::Inputs, Output = Self::Outputs>,
    >(
        &mut self,
        context: &mut ModuleContext<S>,
    ) -> ProcessStatus {
        let block_size = context.stream_data.block_size;
        for out in enum_iter::<Self::Outputs>() {
            context.get_output(out).fill_with(T::zero);
        }

        if let Some(smoothers) = &mut self.smoothers {
            for param in enum_iter::<CartesianProduct<In, Out>>() {
                let smoother = &mut smoothers[param];
                let (in_buf, out_buf) = context.get_input_output_pair(param.0, param.1);
                if self.params[param].has_changed() {
                    smoother.set_target(self.params[param].get_value_normalized());
                }
                // TODO: simd
                for i in 0..block_size {
                    let k = T::cast_from(smoother.next_value());
                    out_buf[i] += k * in_buf[i];
                }
            }
        }

        ProcessStatus::Running
    }
}

#[cfg(test)]
mod tests {
    use crate::r#enum::{CartesianProduct, Enum};
    use az::{Cast, CastFrom};
    use rstest::rstest;
    use std::borrow::Cow;

    use typenum::{Unsigned, U2};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
    enum TestIn {
        A,
        B,
    }

    impl Cast<usize> for TestIn {
        fn cast(self) -> usize {
            match self {
                Self::A => 0,
                Self::B => 1,
            }
        }
    }

    impl CastFrom<usize> for TestIn {
        fn cast_from(src: usize) -> Self {
            match src {
                0 => Self::A,
                1 => Self::B,
                _ => unreachable!(),
            }
        }
    }

    impl Enum for TestIn {
        type Count = U2;

        fn name(&self) -> Cow<str> {
            match self {
                Self::A => Cow::from("A"),
                Self::B => Cow::from("B"),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
    enum TestOut {
        X,
        Y,
    }

    impl Cast<usize> for TestOut {
        fn cast(self) -> usize {
            match self {
                Self::X => 0,
                Self::Y => 1,
            }
        }
    }

    impl CastFrom<usize> for TestOut {
        fn cast_from(src: usize) -> Self {
            match src {
                0 => Self::X,
                1 => Self::Y,
                _ => unreachable!(),
            }
        }
    }

    impl Enum for TestOut {
        type Count = U2;

        fn name(&self) -> Cow<str> {
            match self {
                Self::X => Cow::from("X"),
                Self::Y => Cow::from("Y"),
            }
        }
    }

    #[rstest]
    fn test_param_count() {
        type Params = CartesianProduct<TestIn, TestOut>;
        assert_eq!(<Params as Enum>::Count::USIZE, 4); // TestIn and TestOut have 2 variants each, so 2x2=4
    }
}
