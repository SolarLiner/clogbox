use clogbox_enum::{enum_iter, Empty, Enum};
use clogbox_module::context::ProcessContext;
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Enum)]
pub enum Params {
    Frequency,
    Jitter,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Enum)]
pub enum ParamsOut {
    Tick,
}

#[derive(Debug, Copy, Clone)]
pub struct Clock {
    phase: f32,
    frequency: f32,
    jitter: f32,
    step: f32,
}

impl Module for Clock {
    type Sample = f32;
    type AudioIn = Empty;
    type AudioOut = Empty;
    type ParamsIn = Params;
    type ParamsOut = ParamsOut;
    type NoteIn = Empty;
    type NoteOut = Empty;

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
        self.step = self.frequency * sample_rate.recip() as f32;
        PrepareResult { latency: 0.0 }
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        let mut start = 0;
        let step_recip = self.step.recip();
        while start < context.stream_context.block_size {
            let end = enum_iter::<Params>()
                .filter_map(|p| context.params_in[p].after(start).first().map(|t| t.timestamp))
                .reduce(usize::min)
                .unwrap_or(context.stream_context.block_size);

            // phase + step * (end - start) == 1
            // step * (end - start) == 1 - phase
            // step * end == 1 - phase + step * start
            // end == (1 - phase + step * start) / step
            // end == (1 - phase) / step + start
            let cross = (1.0 - self.phase) * step_recip + start as f32;
            let cross = cross.round() as usize;

            if cross < end {
                // TODO: push tick event
            }

            start = end;
        }
        ProcessResult { tail: None }
    }
}
