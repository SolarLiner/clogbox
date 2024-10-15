use crate::module::{Module, ModuleContext as BaseModuleContext, ProcessStatus, StreamData};
use crate::r#enum::{Enum, EnumMapArray};
use az::CastFrom;
use numeric_array::ArrayLength;

pub type ModuleContext<M> = SampleCtxImpl<<M as SampleModule>::Sample, <M as SampleModule>::Inputs, <M as SampleModule>::Outputs>;

pub struct SampleCtxImpl<T, In: Enum, Out: Enum> where In::Count: ArrayLength, Out::Count: ArrayLength {
    pub stream_data: StreamData,
    pub inputs: EnumMapArray<In, T>,
    pub outputs: EnumMapArray<Out, T>,
}

#[allow(unused_variables)]
pub trait SampleModule: 'static + Send {
    type Sample: Copy + CastFrom<f32> + CastFrom<f64>;
    type Inputs: Enum;
    type Outputs: Enum;

    #[inline]
    fn reallocate(&mut self, stream_data: StreamData) {}

    #[inline]
    fn reset(&mut self) {}

    #[inline]
    fn latency(&self) -> f64 {
        0.0
    }
    
    fn process(&mut self, context: &mut ModuleContext<Self>) -> ProcessStatus where <Self::Inputs as Enum>::Count: ArrayLength, <Self::Outputs as Enum>::Count: ArrayLength;
}

#[profiling::all_functions]
impl<M: SampleModule> Module for M where <M::Inputs as Enum>::Count: ArrayLength, <M::Outputs as Enum>::Count: ArrayLength {
    type Sample = M::Sample;
    type Inputs = M::Inputs;
    type Outputs = M::Outputs;

    #[inline]
    fn supports_stream(&self, _: StreamData) -> bool {
        true
    }

    #[inline]
    fn reallocate(&mut self, stream_data: StreamData) {
        M::reallocate(self, stream_data)
    }

    #[inline]
    fn reset(&mut self) {
        M::reset(self)
    }

    #[inline]
    fn latency(&self) -> f64 {
        M::latency(self)
    }

    fn process(&mut self, context: &mut BaseModuleContext<Self>) -> ProcessStatus {
        let mut status = ProcessStatus::Running;
        for i in 0..context.stream_data.block_size {
            let inputs = EnumMapArray::new(|inp| context.input(inp)[i]);
            let outputs = EnumMapArray::new(|_| M::Sample::cast_from(0.));
            let mut sample_ctx = SampleCtxImpl {
                stream_data: context.stream_data,
                inputs,
                outputs,
            };
            let new_status = M::process(self, &mut sample_ctx);
            for (out, val) in sample_ctx.outputs.iter() {
                context.output(out)[i] = *val;
            }
            
            match new_status {
                ProcessStatus::Running => {}
                new_status @ ProcessStatus::Tail(_) => {
                    status = new_status;
                }
                ProcessStatus::Done => {
                    return ProcessStatus::Done;
                }
            }
        }
        status
    }
}