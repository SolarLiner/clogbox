//! This module provides the core functionalities and structures for handling various
//! audio processing components. It includes definitions for processing statuses,
//! stream metadata, and configuration, as well as implementations of different
//! processing units.

use crate::math::interpolation::{Cubic, InterpolateSingle, Interpolation, Linear};
use crate::module::sample::{SampleContext, SampleModule};
use crate::module::{Module, ModuleConstructor, ProcessStatus, StreamData};
use crate::param;
use crate::param::Params;
use crate::r#enum::{enum_iter, CartesianProduct, Empty, Enum, Mono};
use az::{Cast, CastFrom};
use generic_array::sequence::GenericSequence;
use num_traits::{Float, Num, NumAssign, One, Zero};
use numeric_array::{ArrayLength, NumericArray};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::ops;
use typenum::Unsigned;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct SummingMatrixParams<In, Out>(CartesianProduct<In, Out>);

impl<In, Out> az::CastFrom<usize> for SummingMatrixParams<In, Out>
where
    CartesianProduct<In, Out>: CastFrom<usize>,
{
    fn cast_from(src: usize) -> Self {
        Self(CartesianProduct::cast_from(src))
    }
}

impl<In, Out> az::Cast<usize> for SummingMatrixParams<In, Out>
where
    CartesianProduct<In, Out>: Cast<usize>,
{
    fn cast(self) -> usize {
        self.0.cast()
    }
}

impl<In: Enum, Out: Enum> Enum for SummingMatrixParams<In, Out>
where
    CartesianProduct<In, Out>: Enum,
{
    type Count = <CartesianProduct<In, Out> as Enum>::Count;

    fn name(&self) -> Cow<str> {
        self.0.name()
    }
}

impl<In, Out> Params for SummingMatrixParams<In, Out>
where
    Self: Enum,
{
    fn metadata(&self) -> param::ParamMetadata {
        param::ParamMetadata::CONST_DEFAULT
    }
}

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
    __sample: PhantomData<fn(T, In, Out) -> T>,
}

impl<T, In, Out> Default for SummingMatrix<T, In, Out> {
    fn default() -> Self {
        Self::CONST_DEFAULT
    }
}

impl<T, In, Out> SummingMatrix<T, In, Out> {
    const CONST_DEFAULT: Self = Self {
        __sample: PhantomData,
    };
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
    pub fn new() -> Self {
        Self {
            __sample: PhantomData,
        }
    }
}

impl<
        T: 'static + Copy + Send + NumAssign + Num + Zero + CastFrom<f32>,
        In: 'static + Send + Sync + Enum,
        Out: 'static + Send + Sync + Enum,
    > SampleModule for SummingMatrix<T, In, Out>
where
    In::Count: ops::Mul<Out::Count, Output: Unsigned + ArrayLength>,
{
    type Sample = T;
    type Inputs = In;
    type Outputs = Out;
    type Params = SummingMatrixParams<In, Out>;

    fn latency(&self) -> f64 {
        0.0
    }

    #[inline]
    #[profiling::function]
    fn process_sample(&mut self, mut context: SampleContext<Self>) -> ProcessStatus {
        for out in enum_iter::<Out>() {
            context.outputs[out].set_zero();
        }

        for param in enum_iter::<SummingMatrixParams<In, Out>>() {
            let k = T::cast_from(context.params[param]);
            context.outputs[param.0 .1] += context.inputs[param.0 .0] * k;
        }

        ProcessStatus::Running
    }
}

#[derive(Debug, Clone)]
pub struct FixedDelay<T> {
    delay: VecDeque<T>,
    pos_fract: T,
}

impl<T: Cast<usize> + One + Float> FixedDelay<T> {
    pub fn new(amount: T) -> Self {
        let len = amount.ceil().cast();
        let delay = (0..len).map(|_| T::zero()).collect();
        FixedDelay {
            delay,
            pos_fract: T::one() - amount.fract(),
        }
    }
}

impl<T: 'static + Copy + Send + Float + Cast<usize>> SampleModule for FixedDelay<T> {
    type Sample = T;
    type Inputs = Mono;
    type Outputs = Mono;
    type Params = Empty;

    fn process_sample(&mut self, mut context: SampleContext<Self>) -> ProcessStatus {
        use self::Mono::Mono;

        context.outputs[Mono] =
            Linear.interpolate_single(&NumericArray::generate(|i| self.delay[i]), self.pos_fract);
        self.delay.pop_front().unwrap();
        self.delay.push_back(context.inputs[Mono]);
        ProcessStatus::Tail(self.delay.len() as _)
    }
}

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Deserialize, serde::Serialize))]
pub struct FixedDelayConstructor<T> {
    pub amount: f64,
    __sample: PhantomData<T>,
}

impl<T: One + Float + Cast<usize> + CastFrom<f64>> ModuleConstructor for FixedDelayConstructor<T>
where
    FixedDelay<T>: Module,
{
    type Module = FixedDelay<T>;

    fn allocate(&self, stream_data: StreamData) -> Self::Module {
        FixedDelay::new(T::cast_from(self.amount))
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
