//! This module provides the core functionalities and structures for handling various 
//! audio processing components. It includes definitions for processing statuses, 
//! stream metadata, and configuration, as well as implementations of different 
//! processing units.
use crate::module::{Module, ModuleContext, ProcessStatus, StreamData};
use crate::param::curve::ParamCurve;
use crate::r#enum::{enum_iter, CartesianProduct, Enum};
use num_traits::{Num, NumAssign, Zero};
use numeric_array::ArrayLength;
use std::marker::PhantomData;
use std::ops;
use az::CastFrom;
use typenum::Unsigned;
use crate::r#enum::enum_map::{EnumMap, EnumMapArray, EnumMapBox};

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
    params: EnumMapBox<CartesianProduct<In, Out>, ParamCurve>,
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
        sample_rate: f32,
        max_timestamps: usize,
        initial_values: EnumMapBox<CartesianProduct<In, Out>, f32>,
    ) -> Self {
        Self {
            params: EnumMap::new(|k| {
                ParamCurve::new(sample_rate, max_timestamps, initial_values[k])
            }),
            __sample: PhantomData,
        }
    }

    /// Mutably borrows the `ParamCurve` associated with the given input-output pair.
    ///
    /// # Parameters
    /// * `inp` - The input parameter.
    /// * `out` - The output parameter.
    ///
    /// # Returns
    /// 
    /// A mutable reference to the `ParamCurve` corresponding to the input-output combination.
    pub fn param_block_mut(&mut self, inp: In, out: Out) -> &mut ParamCurve {
        &mut self.params[CartesianProduct(inp, out)]
    }
}

impl<
        T: 'static + Copy + Send + NumAssign + Num + Zero + CastFrom<f32>,
        In: 'static + Enum,
        Out: 'static + Enum,
    > Module for SummingMatrix<T, In, Out>
where
    In::Count: ops::Mul<Out::Count, Output: Unsigned + ArrayLength>,
{
    type Sample = T;
    type Inputs = In;
    type Outputs = Out;

    fn supports_stream(&self, _: StreamData) -> bool {
        true
    }

    fn reallocate(&mut self, stream_data: StreamData) {
        self.params = EnumMap::new(|k| ParamCurve::new(stream_data.sample_rate as _, Self::PARAMS_MAX_TIMESTAMPS, self.params[k].last_value()));
    }

    fn latency(&self, input_latencies: EnumMapArray<Self::Inputs, f64>) -> EnumMapArray<Self::Outputs, f64> {
        EnumMapArray::new(|out| input_latencies.iter().map(|(k, &v)| v * self.params[CartesianProduct(k, out)].last_value() as f64).sum())
    }

    #[inline]
    #[profiling::function]
    fn process(&mut self, context: &mut ModuleContext<Self>) -> ProcessStatus {
        let block_size = context.stream_data.block_size;
        for x in context.outputs.values_mut() {
            x.fill(T::zero());
        }

        for param in enum_iter::<CartesianProduct<In, Out>>() {
            let in_buf = context.inputs[param.0];
            let out_buf = &mut *context.outputs[param.1];
            let parr = &self.params[param];
            // TODO: simd
            for i in 0..block_size {
                let k = T::cast_from(parr.get_value_sample(i));
                out_buf[i] += k * in_buf[i];
            }
        }

        ProcessStatus::Running
    }
}

/// A struct for running two modules in series.
///
/// This struct contains two audio modules (`first` and `second`) and a switch function
/// that defines the inputs of the second module from the outputs of the first.
#[derive(Debug, Clone)]
pub struct Series<A: Module, B: Module<Sample = A::Sample>, SwitchFn> {
    /// The first audio module in the series.
    pub first: A,
    /// The second audio module in the series.
    pub second: B,
    inner_buffer: EnumMapArray<A::Outputs, Box<[A::Sample]>>,
    switch_fn: SwitchFn,
}

impl<
        A: Module,
        B: Module<Sample = A::Sample, Inputs = A::Outputs>,
        SwitchFn: Send + 'static + Fn(B::Inputs) -> A::Outputs,
    > Module for Series<A, B, SwitchFn>
where
    A::Sample: Send + Zero,
{
    type Sample = A::Sample;
    type Inputs = A::Inputs;
    type Outputs = B::Outputs;

    fn supports_stream(&self, data: StreamData) -> bool {
        self.inner_buffer
            .iter()
            .all(|(_, arr)| data.block_size <= arr.len())
            && self.first.supports_stream(data)
            && self.second.supports_stream(data)
    }

    fn reallocate(&mut self, stream_data: StreamData) {
        self.inner_buffer = EnumMapArray::new(|_| {
            std::iter::repeat_with(A::Sample::zero)
                .take(stream_data.block_size)
                .collect()
        });
    }

    fn reset(&mut self) {
        for x in self.inner_buffer.values_mut() {
            x.fill_with(A::Sample::zero);
        }
    }

    fn latency(&self, input_latencies: EnumMapArray<Self::Inputs, f64>) -> EnumMapArray<Self::Outputs, f64> {
        let first = self.first.latency(input_latencies);
        let second_input = EnumMapArray::new(|in_b| first[(self.switch_fn)(in_b)]);
        self.second.latency(second_input)
    }

    fn process(&mut self, context: &mut ModuleContext<Self>) -> ProcessStatus {
        todo!()
    }
}
