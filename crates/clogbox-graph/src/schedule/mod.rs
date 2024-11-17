use crate::graph::{ModuleMap, NodeID, ScheduleEntry};
use crate::{graph, PortType};
use clogbox_core::module::{BufferStorage, MappedBufferStorage, ModuleContext, ProcessStatus, RawModule, StreamData};
use clogbox_core::r#enum::Either;
use num_traits::Zero;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::Arc;
use clogbox_core::param::{RawParams, EMPTY_PARAMS};

#[cfg(feature = "serialize")]
pub mod serde;

pub struct Schedule<T> {
    pub(crate) schedule: graph::CompiledSchedule<PortType>,
    pub(crate) input_nodes: HashSet<NodeID>,
    pub(crate) output_nodes: HashSet<NodeID>,
    pub(crate) modules: ModuleMap<T>,
    pub(crate) buffers: Box<[Box<[T]>]>,
    pub(crate) max_buffer_size: usize,
}

impl<T> PartialEq for Schedule<T> {
    fn eq(&self, other: &Self) -> bool {
        for (a, b) in self.schedule.schedule.iter().zip(other.schedule.schedule.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
}

impl<T: Send + Zero> RawModule for Schedule<T> {
    type Sample = T;

    fn inputs(&self) -> usize {
        self.input_nodes.len()
    }

    fn outputs(&self) -> usize {
        self.output_nodes.len()
    }

    fn get_params(&self) -> Arc<dyn '_ + RawParams> {
        Arc::new(EMPTY_PARAMS)
    }

    fn supports_stream(&self, data: StreamData) -> bool {
        self.max_buffer_size >= data.block_size
            && self.modules.values().all(|m| m.supports_stream(data))
    }

    fn reallocate(&mut self, stream_data: StreamData) {
        let max_buffers = self.schedule.num_buffers[PortType::Audio];
        self.buffers = std::iter::repeat_with(|| {
            std::iter::repeat_with(T::zero)
                .take(stream_data.block_size)
                .collect::<Box<[_]>>()
        })
        .take(max_buffers)
        .collect::<Box<[_]>>();
    }

    fn reset(&mut self) {}

    fn process(
        &mut self,
        context: &mut ModuleContext<
            &mut dyn BufferStorage<Sample = Self::Sample, Input = usize, Output = usize>,
        >,
    ) -> ProcessStatus {
        for entry in &self.schedule.schedule {
            match entry {
                ScheduleEntry::Node(node) => {
                    for assignment in &node.input_buffers {
                        if assignment.should_clear {
                            self.buffers.clear_input(assignment.buffer_index.0);
                        }
                    }
                    for assignment in &node.output_buffers {
                        if assignment.should_clear {
                            self.buffers.clear_output(assignment.buffer_index.0);
                        }
                    }
                    let mut storage = MappedBufferStorage {
                        storage: &mut *self.buffers,
                        mapper: |x: Either<usize, usize>| match x {
                            Either::Left(input) => node.input_buffers[input].buffer_index.0,
                            Either::Right(output) => node.output_buffers[output].buffer_index.0,
                        },
                        __io_types: PhantomData,
                    };
                    self.modules[node.id].process(&mut ModuleContext {
                        stream_data: context.stream_data,
                        buffers: &mut storage,
                    });
                }
                ScheduleEntry::Delay(_) => {}
                ScheduleEntry::Sum(_) => {}
            }
        }
        ProcessStatus::Running
    }
}