//! # `fundsp` integration for `clogbox`
//!
//! This module provides interoperability between `fundsp` and `clogbox`. This allows one to operate with the other's
//! framework.
//!
//! This means you can create `fundsp` audio graphs and turn them into [`SampleModule`]s. The integration works in
//! the other direction as well, allowing you to take [`SampleModule`]s and use them as a `fundsp` [`AudioNode`]s.
use crate::context::StreamContext;
use crate::sample::{SampleModule, SampleProcessResult};
use crate::{PrepareResult, Samplerate};
use clogbox_enum::enum_map::{EnumMapArray, EnumMapMut, EnumMapRef};
use clogbox_enum::{Count, Empty, Enum, Sequential};
use fundsp::prelude::*;

/// Wrap a [`fundsp::AudioNode`] into a [`SampleModule`].
pub struct FundspModule<N: AudioNode, Params: Enum = Empty> {
    params: EnumMapArray<Params, Shared>,
    node: An<N>,
}

impl<N: AudioNode, Params: Enum> SampleModule for FundspModule<N, Params> {
    type Sample = f32;
    type AudioIn = Sequential<N::Inputs>;
    type AudioOut = Sequential<N::Outputs>;
    type Params = Params;

    fn prepare(&mut self, sample_rate: Samplerate) -> PrepareResult {
        self.node.set_sample_rate(sample_rate.value());
        self.node.allocate();
        let latency = self.node.latency().unwrap_or(0.0);
        PrepareResult { latency }
    }

    fn process(
        &mut self,
        _stream_context: &StreamContext,
        inputs: EnumMapArray<Self::AudioIn, Self::Sample>,
        params: EnumMapRef<Self::Params, f32>,
    ) -> SampleProcessResult<Self::AudioOut, Self::Sample> {
        for (p, shared) in self.params.iter_mut() {
            shared.set_value(params[p]);
        }
        let outputs = self.node.tick(Frame::from_slice(inputs.as_slice()));
        SampleProcessResult {
            tail: None,
            output: EnumMapArray::new(|e: Sequential<N::Outputs>| outputs[e.to_usize()]),
        }
    }
}

impl<N: AudioNode, Params: Enum> FundspModule<N, Params> {
    /// Create a new [`FundspModule`] from the provided audio node, and parameters, given as [`Shared`] values.
    ///
    /// To have the [`Shared`] values provided to you, use [`Self::create`] instead.
    ///
    /// # Arguments
    ///
    /// * `node`: `fundsp` node to wrap as a [`SampleModule`].
    /// * `params`: List of [`Shared`] values to track as parameters in this module. All [`Shared`] values will be
    /// automatically updated with the values of the incoming parameters. Use those [`Shared`] values in your graph
    /// to benefit from the automatic updates.
    pub fn new(node: An<N>, params: EnumMapArray<Params, Shared>) -> Self {
        Self { params, node }
    }

    /// Create a `fundsp` graph given an [`EnumMap`] of [`Shared`] values.
    ///
    /// # Arguments
    ///
    /// * `gen`: Function defining the audio graph. An [`EnumMap`] of [`Shared`] values associated with `Params` enum
    /// will be provided for you to use in the graph. These [`Shared`] values are automatically synchronized with the
    /// module parameters. Use those values in your graph to benefit from the automatic updates.
    pub fn create(gen: impl FnOnce(EnumMapMut<Params, Shared>) -> An<N>) -> Self {
        let mut params = EnumMapArray::new(|_| shared(0.0));
        let node = gen(params.to_mut());
        Self::new(node, params)
    }
}

/// A [`fundsp::AudioNode`] which wraps a [`SampleModule`] instance.
#[derive(Clone)]
pub struct ClogboxNode<SM: SampleModule> {
    /// Inner `clogbox` module.
    pub module: SM,
    params: EnumMapArray<SM::Params, Shared>,
    stream_context: StreamContext,
    latency: f64,
}

impl<SM: SampleModule> ClogboxNode<SM> {
    /// Create a new [`ClogboxNode`] given a [`SampleModule`] instance and default parameters associated with the
    /// module to wrap.
    ///
    /// # Arguments
    ///
    /// * `module`: `clogbox` module to be wrapped
    /// * `default_params`: default values for the parameters of the module
    pub fn new(module: SM, default_params: EnumMapArray<SM::Params, f32>) -> Self {
        let params = EnumMapArray::new(|p| shared(default_params[p]));
        let stream_context = StreamContext {
            sample_rate: Samplerate::new(1.0),
            block_size: 1,
        };
        Self {
            module,
            params,
            stream_context,
            latency: 0.0,
        }
    }

    /// Returns the corresponding [`Shared`] instance for a specific parameter.
    pub fn shared(&self, param: SM::Params) -> &Shared {
        &self.params[param]
    }
}

impl<SM: Send + Sync + Clone + SampleModule<Sample = f32, Params: Sync + Send>> AudioNode for ClogboxNode<SM>
where
    <<SM as SampleModule>::AudioIn as Enum>::Count: Send + Sync,
    <<SM as SampleModule>::AudioOut as Enum>::Count: Send + Sync,
{
    const ID: u64 = 0;
    type Inputs = Count<SM::AudioIn>;
    type Outputs = Count<SM::AudioOut>;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.stream_context.sample_rate = Samplerate::new(sample_rate);
    }

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let inputs = EnumMapArray::new(|p: SM::AudioIn| input[p.to_usize()]);
        let params = EnumMapArray::new(|p| self.params[p].value());
        let outputs = self.module.process(&self.stream_context, inputs, params.to_ref());
        Frame::from_slice(outputs.output.as_slice()).clone()
    }

    fn allocate(&mut self) {
        let PrepareResult { latency } = self.module.prepare(self.stream_context.sample_rate);
        self.latency = latency;
    }

    fn latency(&mut self) -> Option<f64> {
        Some(self.latency)
    }
}
