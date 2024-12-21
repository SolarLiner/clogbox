use crate::graph::context::{GraphContextImpl, RawGraphContext};
use crate::graph::module::{ModuleError, RawModule};
use crate::graph::r#impl::{BufferAssignment, BufferIdx, EdgeID, NodeID, ScheduleEntry};
use crate::graph::storage::GraphStorage;
use crate::graph::{r#impl, SlotType};
use crate::module::{ProcessStatus, StreamData};
use crate::modules::delay::FixedAudioDelay;
use crate::modules::sum::Sum;
use az::Cast;
use derive_more::Debug;
use num_traits::{Float, Zero};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::ops;

/// Type that drives a graph schedule as a module.
#[derive(Debug)]
pub struct GraphDriver<T> {
    input_nodes: SparseSecondaryMap<NodeID, usize>,
    output_nodes: SparseSecondaryMap<NodeID, usize>,
    #[debug(skip)]
    nodes: SecondaryMap<NodeID, Box<dyn RawModule<Sample = T> + Send + Sync>>,
    storage: GraphStorage<T>,
    schedule: r#impl::CompiledSchedule<SlotType>,
    delays: SparseSecondaryMap<EdgeID, FixedAudioDelay<T>>,
}

impl<T: Copy + Float + ops::AddAssign<T> + Cast<usize>> GraphDriver<T> {
    pub fn process(
        &mut self,
        stream_data: &StreamData,
        inputs: &[&[T]],
        outputs: &mut [&mut [T]],
    ) -> Result<ProcessStatus, ModuleError> {
        let schedule = std::mem::take(&mut self.schedule.schedule);
        for entry in schedule.iter() {
            match entry {
                ScheduleEntry::Node(node) => {
                    self.preprocess_buffers(&node.input_buffers);
                    self.preprocess_buffers(&node.output_buffers);
                    let get_input_index = |slot_type: SlotType, id: usize| {
                        let buf = &node.input_buffers.get(id)?;
                        if buf.type_index != slot_type {
                            return None;
                        }
                        Some((slot_type, buf.buffer_index.0))
                    };
                    let get_output_index = |slot_type: SlotType, id: usize| {
                        let buf = &node.output_buffers.get(id)?;
                        if buf.type_index != slot_type {
                            return None;
                        }
                        Some((slot_type, buf.buffer_index.0))
                    };

                    if self.input_nodes.contains_key(node.id) {
                        let input_ix = self.input_nodes[node.id];
                        let BufferIdx(idx) = node.output_buffers[0].buffer_index;
                        let mut buf = self
                            .storage
                            .get_buffer_mut(SlotType::Audio, idx)
                            .map(|m| m.to_audio_buffer())
                            .transpose()
                            .unwrap();
                        buf.copy_from_slice(&inputs[input_ix]);
                    } else if self.output_nodes.contains_key(node.id) {
                        let output_ix = self.output_nodes[node.id];
                        let BufferIdx(idx) = node.input_buffers[0].buffer_index;
                        let buf = self
                            .storage
                            .get_buffer(SlotType::Audio, idx)
                            .map(|r| r.to_audio_buffer())
                            .transpose()
                            .unwrap();
                        outputs[output_ix].copy_from_slice(&buf);
                    } else {
                        self.nodes[node.id].process(RawGraphContext {
                            storage: &mut self.storage,
                            input_index: &get_input_index,
                            output_index: &get_output_index,
                            stream_data,
                        })?;
                    }
                }
                ScheduleEntry::Delay(delay) => {
                    let get_input_index = |slot_type: SlotType, id: usize| {
                        Some((slot_type, delay.input_buffer.buffer_index.0))
                    };
                    let get_output_index = |slot_type: SlotType, id: usize| {
                        Some((slot_type, delay.output_buffer.buffer_index.0))
                    };
                    self.delays[delay.edge.id].process(RawGraphContext {
                        storage: &self.storage,
                        input_index: &get_input_index,
                        output_index: &get_output_index,
                        stream_data,
                    })?;
                }
                ScheduleEntry::Sum(sum) => {
                    let slot_type = sum.output_buffer.type_index;
                    let mut out = self
                        .storage
                        .get_buffer_mut(slot_type, sum.output_buffer.buffer_index.0);
                    match slot_type {
                        SlotType::Audio => {
                            let mut buf = out.map(|m| m.to_audio_buffer()).transpose().unwrap();
                            buf.fill_with(T::zero);
                            for inp in &sum.input_buffers {
                                let inp = self
                                    .storage
                                    .get_buffer(SlotType::Audio, inp.buffer_index.0)
                                    .map(|r| r.to_audio_buffer())
                                    .transpose()
                                    .unwrap();
                                for i in 0..stream_data.block_size {
                                    buf[i] += inp[i];
                                }
                            }
                        }
                        SlotType::Control => {
                            let mut buf = out.map(|m| m.to_control_events()).transpose().unwrap();
                            buf.clear();
                            for inp in &sum.input_buffers {
                                let inp = self
                                    .storage
                                    .get_buffer(SlotType::Control, inp.buffer_index.0)
                                    .map(|r| r.to_control_events())
                                    .transpose()
                                    .unwrap();
                                for ev in inp.iter_events() {
                                    buf.push(ev.sample, *ev.value);
                                }
                            }
                        }
                        SlotType::Note => {
                            let mut buf = out.map(|m| m.to_note_events()).transpose().unwrap();
                            buf.clear();
                            for inp in &sum.input_buffers {
                                let inp = self
                                    .storage
                                    .get_buffer(SlotType::Note, inp.buffer_index.0)
                                    .map(|r| r.to_note_events())
                                    .transpose()
                                    .unwrap();
                                for ev in inp.iter_events() {
                                    let _ = buf.push(ev.sample, *ev.value);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(ProcessStatus::Running)
    }

    fn preprocess_buffers(&mut self, assignments: &[BufferAssignment<SlotType>]) {
        for assignment in assignments {
            if assignment.should_clear {
                self.storage
                    .get_buffer_mut(assignment.type_index, assignment.buffer_index.0)
                    .clear()
            }
        }
    }
}
