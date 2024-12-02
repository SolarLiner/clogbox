use crate::graph::{EdgeID, ModuleMap, NodeID, ScheduleEntry};
use crate::{graph, PortType};
use arrayvec::ArrayVec;
use clogbox_core::module::utilitarian::FixedDelay;
use clogbox_core::module::{
    BufferStorage, MappedBufferStorage, Module, ModuleContext, ProcessStatus, RawModule,
    SingleBufferStorage, StreamData,
};
use clogbox_core::param::container::{MappedContainer, ParamEventsContainer};
use clogbox_core::param::events::{ParamEvents, ParamSlice};
use clogbox_core::r#enum::enum_map::EnumMapRef;
use clogbox_core::r#enum::Either;
use num_traits::Zero;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::ops;

#[cfg(feature = "serialize")]
pub mod serde;

pub struct Schedule<T> {
    pub(crate) schedule: graph::CompiledSchedule<PortType>,
    pub(crate) inputs: HashMap<NodeID, usize>,
    pub(crate) outputs: HashMap<NodeID, usize>,
    pub(crate) param_nodes: HashMap<NodeID, usize>,
    pub(crate) modules: ModuleMap<T>,
    pub(crate) buffers_audio: Box<[Box<[T]>]>,
    pub(crate) buffers_param: Box<[Box<ParamSlice>]>,
    pub(crate) max_buffer_size: usize,
    pub(crate) audio_delays: HashMap<EdgeID, FixedDelay<T>>,
}

impl<T> PartialEq for Schedule<T> {
    fn eq(&self, other: &Self) -> bool {
        for (a, b) in self
            .schedule
            .schedule
            .iter()
            .zip(other.schedule.schedule.iter())
        {
            if a != b {
                return false;
            }
        }
        true
    }
}

impl<T: Send + Copy + Zero + ops::AddAssign<T>> RawModule for Schedule<T>
where
    FixedDelay<T>: RawModule<Sample = T>,
{
    type Sample = T;

    fn inputs(&self) -> usize {
        self.inputs.len()
    }

    fn outputs(&self) -> usize {
        self.outputs.len()
    }

    fn params(&self) -> usize {
        self.param_nodes.len()
    }

    fn supports_stream(&self, data: StreamData) -> bool {
        self.max_buffer_size >= data.block_size
            && self.modules.values().all(|m| m.supports_stream(data))
    }

    fn reallocate(&mut self, stream_data: StreamData) {
        self.buffers_audio = std::iter::repeat_with(|| {
            std::iter::repeat_with(T::zero)
                .take(stream_data.block_size)
                .collect::<Box<[_]>>()
        })
        .take(self.schedule.num_buffers[PortType::Audio])
        .collect::<Box<[_]>>();
        self.buffers_param = std::iter::repeat_with(|| {
            std::iter::repeat((0, 0.0))
                .take(stream_data.block_size)
                .collect::<Box<[_]>>()
        })
        .take(self.schedule.num_buffers[PortType::Param])
        .collect::<Box<[_]>>();
    }

    fn reset(&mut self) {}

    fn process(
        &mut self,
        context: &mut ModuleContext<
            &mut dyn BufferStorage<Sample = Self::Sample, Input = usize, Output = usize>,
        >,
        params: &dyn ParamEventsContainer<usize>,
    ) -> ProcessStatus {
        for entry in &self.schedule.schedule {
            match entry {
                ScheduleEntry::Node(node) => {
                    for assignment in &node.input_buffers {
                        match assignment.type_index {
                            PortType::Audio => {
                                if assignment.should_clear {
                                    self.buffers_audio.clear_input(assignment.buffer_index.0);
                                }
                            }
                            PortType::Param => {
                                if assignment.should_clear {
                                    self.buffers_param[assignment.buffer_index.0].fill((0, 0.0));
                                }
                            }
                        }
                    }
                    for assignment in &node.output_buffers {
                        match assignment.type_index {
                            PortType::Audio => {
                                if assignment.should_clear {
                                    self.buffers_audio.clear_output(assignment.buffer_index.0);
                                }
                            }
                            PortType::Param => {
                                if assignment.should_clear {
                                    self.buffers_param[assignment.buffer_index.0].fill((0, 0.0));
                                }
                            }
                        }
                    }
                    let block_size = context.stream_data.block_size;
                    if let Some(&input_index) = self.inputs.get(&node.id) {
                        self.buffers_audio[node.output_buffers[0].buffer_index.0][..block_size]
                            .copy_from_slice(context.get_input(input_index))
                    } else if let Some(&output_index) = self.outputs.get(&node.id) {
                        context.get_output(output_index).copy_from_slice(
                            &self.buffers_audio[node.input_buffers[0].buffer_index.0][..block_size],
                        )
                    } else {
                        let mut storage = MappedBufferStorage {
                            storage: &mut *self.buffers_audio,
                            mapper: |x: Either<usize, usize>| match x {
                                Either::Left(input) => node.input_buffers[input].buffer_index.0,
                                Either::Right(output) => node.output_buffers[output].buffer_index.0,
                            },
                            __io_types: PhantomData,
                        };
                        let module = &mut *self.modules[node.id];
                        let params = MappedContainer::new(&self.buffers_param, |out: usize| {
                            node.input_buffers[out].buffer_index.0
                        });
                        module.process(
                            &mut ModuleContext {
                                stream_data: context.stream_data,
                                buffers: &mut storage,
                            },
                            &params,
                        );
                    }
                }
                ScheduleEntry::Delay(delay) => match delay.input_buffer.type_index {
                    PortType::Audio => {
                        let input = delay.input_buffer.buffer_index.0;
                        let output = delay.output_buffer.buffer_index.0;
                        let (input, output) =
                            get_element_pair_mut(&mut self.buffers_audio, input, output);
                        let (input, output) = (&mut **input, &mut **output);
                        let mut buffers = SingleBufferStorage { input, output };
                        let params: &[f32] = &[];
                        self.audio_delays.entry(delay.edge.id).and_modify(|d| {
                            d.process(
                                &mut ModuleContext {
                                    stream_data: context.stream_data,
                                    buffers: &mut buffers,
                                },
                                &params,
                            );
                        });
                    }
                    PortType::Param => {
                        let input = delay.input_buffer.buffer_index.0;
                        let output = delay.output_buffer.buffer_index.0;
                        let (input, output) =
                            get_element_pair_mut(&mut self.buffers_param, input, output);
                        for (out, (input_pos, input_value)) in
                            output.iter_mut().zip(input.iter().copied())
                        {
                            *out = (input_pos + delay.delay.round() as usize, input_value);
                        }
                    }
                },
                ScheduleEntry::Sum(sum) => {
                    if sum.output_buffer.type_index == PortType::Param {
                        eprintln!("Summing parameters is not supported");
                        continue;
                    }
                    let block_size = context.stream_data.block_size;
                    let mut it = sum.input_buffers.iter();
                    let Some(next) = it.next() else {
                        continue;
                    };
                    let (input, output) = get_element_pair_mut(
                        &mut self.buffers_audio,
                        next.buffer_index.0,
                        sum.output_buffer.buffer_index.0,
                    );
                    output[..block_size].copy_from_slice(&input[..block_size]);

                    for next in it {
                        let (input, output) = get_element_pair_mut(
                            &mut self.buffers_audio,
                            next.buffer_index.0,
                            sum.output_buffer.buffer_index.0,
                        );
                        for i in 0..block_size {
                            output[i] += input[i];
                        }
                    }
                }
            }
        }
        ProcessStatus::Running
    }
}

fn get_element_pair_mut<T>(arr: &mut [T], ix_a: usize, ix_b: usize) -> (&mut T, &mut T) {
    if ix_a < ix_b {
        let (a, b) = arr.split_at_mut(ix_b);
        (&mut a[ix_a], &mut b[0])
    } else {
        let (a, b) = arr.split_at_mut(ix_a);
        (&mut b[0], &mut a[ix_b])
    }
}
