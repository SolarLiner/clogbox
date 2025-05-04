use crate::eventbuffer::Timestamped;
use crate::{Module, PrepareResult, ProcessContext, ProcessResult, Samplerate, StreamContext};
use clogbox_enum::enum_map::{EnumMapArray, EnumMapRef};
use clogbox_enum::{enum_iter, Empty, Enum};
use std::num::NonZeroU32;

pub struct SampleProcessResult<E: Enum, T> {
    pub tail: Option<NonZeroU32>,
    pub output: EnumMapArray<E, T>,
}

pub trait SampleModule {
    type Sample;
    type AudioIn: Enum;
    type AudioOut: Enum;
    type Params: Enum;

    fn prepare(&mut self, sample_rate: Samplerate) -> PrepareResult;
    fn process(
        &mut self,
        stream_context: &StreamContext,
        inputs: EnumMapArray<Self::AudioIn, Self::Sample>,
        params: EnumMapRef<Self::Params, f32>,
    ) -> SampleProcessResult<Self::AudioOut, Self::Sample>;
}

pub struct SampleModuleWrapper<SM: SampleModule> {
    pub sample_module: SM,
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

        result
    }
}

impl<SM: SampleModule> SampleModuleWrapper<SM> {
    pub fn new(sample_module: SM, params: EnumMapArray<SM::Params, f32>) -> Self {
        Self { params, sample_module }
    }
}
