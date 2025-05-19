//! # Sample-by-sample processing abstractions
//!
//! This module provides traits and types for implementing modules that process
//! audio one sample at a time, as opposed to block processing.

use crate::context::{ProcessContext, StreamContext};
use crate::eventbuffer::Timestamped;
use crate::{Module, PrepareResult, ProcessResult, Samplerate};
use clogbox_enum::enum_map::{EnumMapArray, EnumMapRef};
use clogbox_enum::{enum_iter, Empty, Enum};
use std::num::NonZeroU32;

/// Result of processing a single audio sample.
///
/// This struct contains the output audio sample and information about
/// the module's tail, similar to the `ProcessResult` for block processing.
pub struct SampleProcessResult<E: Enum, T> {
    /// The number of additional blocks the module needs to output its tail.
    ///
    /// This is used for modules that produce output even after their input has stopped,
    /// such as reverbs and delays. If `None`, the module has no tail.
    pub tail: Option<NonZeroU32>,

    /// The output audio sample for each output channel.
    pub output: EnumMapArray<E, T>,
}

/// Trait for modules that process audio one sample at a time.
///
/// This trait defines the interface for audio processing modules that operate
/// on individual samples rather than blocks of audio. It's useful for simpler
/// DSP algorithms or when sample-by-sample processing is required.
pub trait SampleModule {
    /// The sample type used by this module (typically f32 or f64).
    type Sample;

    /// The enum type defining audio input channels.
    type AudioIn: Enum;

    /// The enum type defining audio output channels.
    type AudioOut: Enum;

    /// The enum type defining parameter input types.
    type Params: Enum;

    /// Prepares the module for processing at the given sample rate.
    ///
    /// This method is called before processing begins or when the sample rate
    /// changes, allowing the module to initialize its internal state.
    ///
    /// # Parameters
    ///
    /// * `sample_rate` - The sample rate at which the module will operate
    ///
    /// # Returns
    ///
    /// A `PrepareResult` containing information about the module's state after preparation.
    fn prepare(&mut self, sample_rate: Samplerate) -> PrepareResult;

    /// Called at the beginning of each processing block.
    ///
    /// This method allows the module to perform any setup needed before
    /// processing a block of samples. The default implementation does nothing.
    ///
    /// # Parameters
    ///
    /// * `stream_context` - Information about the current processing stream
    #[allow(unused_variables)]
    fn on_block_begin(&mut self, stream_context: &StreamContext) {}

    /// Processes a single audio sample.
    ///
    /// This method is called for each sample to be processed. It receives
    /// the current input sample and parameters, and produces an output sample.
    ///
    /// # Parameters
    ///
    /// * `stream_context` - Information about the current processing stream
    /// * `inputs` - The input audio sample for each input channel
    /// * `params` - The current parameter values
    ///
    /// # Returns
    ///
    /// A `SampleProcessResult` containing the output sample and tail information.
    fn process(
        &mut self,
        stream_context: &StreamContext,
        inputs: EnumMapArray<Self::AudioIn, Self::Sample>,
        params: EnumMapRef<Self::Params, f32>,
    ) -> SampleProcessResult<Self::AudioOut, Self::Sample>;

    /// Called at the end of each processing block.
    ///
    /// This method allows the module to perform any cleanup needed after
    /// processing a block of samples. The default implementation does nothing.
    ///
    /// # Parameters
    ///
    /// * `stream_context` - Information about the current processing stream
    #[allow(unused_variables)]
    fn on_block_end(&mut self, stream_context: &StreamContext) {}
}

/// A wrapper that adapts a `SampleModule` to the `Module` trait.
///
/// This struct allows sample-by-sample processing modules to be used
/// in contexts that expect block processing modules. It handles the
/// conversion between block and sample processing.
pub struct SampleModuleWrapper<SM: SampleModule> {
    /// The underlying sample module being wrapped.
    pub sample_module: SM,

    /// The current parameter values for the sample module.
    params: EnumMapArray<SM::Params, f32>,
}

impl<SM: SampleModule<Sample: Copy>> Module for SampleModuleWrapper<SM> {
    type Sample = SM::Sample;
    type AudioIn = SM::AudioIn;
    type AudioOut = SM::AudioOut;
    type ParamsIn = SM::Params;
    type ParamsOut = Empty;
    type NoteIn = Empty;
    type NoteOut = Empty;

    fn prepare(&mut self, sample_rate: Samplerate, _block_size: usize) -> PrepareResult {
        self.sample_module.prepare(sample_rate)
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        let mut result = ProcessResult { tail: None };

        self.sample_module.on_block_begin(context.stream_context);
        for i in 0..context.stream_context.block_size {
            // Update params
            for p in enum_iter::<Self::ParamsIn>() {
                let Some(&Timestamped { data: value, .. }) = context.params_in[p].at(i) else {
                    continue;
                };
                self.params[p] = value;
            }

            // Process sample
            let inputs = EnumMapArray::new(|e| context.audio_in[e][i]);
            let SampleProcessResult { tail, output } =
                self.sample_module
                    .process(context.stream_context, inputs, self.params.to_ref());
            result.tail = tail;
            for (e, out) in output {
                context.audio_out[e][i] = out;
            }
        }
        self.sample_module.on_block_end(context.stream_context);

        result
    }
}

impl<SM: SampleModule> SampleModuleWrapper<SM> {
    /// Creates a new `SampleModuleWrapper` with the given sample module and initial parameters.
    ///
    /// # Parameters
    ///
    /// * `sample_module` - The sample module to wrap
    /// * `params` - The initial parameter values for the sample module
    ///
    /// # Returns
    ///
    /// A new `SampleModuleWrapper` instance
    pub fn new(sample_module: SM, params: EnumMapArray<SM::Params, f32>) -> Self {
        Self { params, sample_module }
    }
}
