use crate::graph::context::{GraphContext, GraphContextImpl, RawGraphContext};
use crate::graph::slots::Slots;
use crate::module::ProcessStatus;
use az::Cast;
use std::error::Error;
use thiserror::Error;
use crate::graph::SlotType;

#[derive(Debug, Error)]
pub enum ModuleError {
    #[error("Missing required input: {1} of type {0:?}")]
    MissingRequiredInput(SlotType, String),
    #[error("Missing required output: {1} of type {0:?}")]
    MissingRequiredOutput(SlotType, String),
    #[error("Module not ready")]
    NotReady,
    #[error("Fatal error: {0}")]
    Fatal(#[from] Box<dyn Error>),
}

pub trait Module {
    type Sample;
    type Inputs: Slots;
    type Outputs: Slots;

    fn process(&mut self, graph_context: GraphContext<Self>) -> Result<ProcessStatus, ModuleError>;
}

pub trait RawModule {
    type Sample;

    fn process(
        &mut self,
        graph_context: RawGraphContext<Self::Sample>,
    ) -> Result<ProcessStatus, ModuleError>;
}

impl<M: Module> RawModule for M {
    type Sample = M::Sample;

    fn process(
        &mut self,
        graph_context: RawGraphContext<Self::Sample>,
    ) -> Result<ProcessStatus, ModuleError> {
        let input_index = |id: M::Inputs| (graph_context.input_index)(id.slot_type(), id.cast());
        let output_index = |id: M::Outputs| (graph_context.output_index)(id.slot_type(), id.cast());
        Module::process(
            self,
            GraphContextImpl {
                stream_data: graph_context.stream_data,
                input_index: &input_index,
                output_index: &output_index,
                storage: graph_context.storage,
            },
        )
    }
}
