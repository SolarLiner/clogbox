//! Output data structures from the audio graph compiler.

use std::hash::Hash;

use super::input_ir::{Edge, NodeID, PortID};

use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::Enum;
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

/// The index of the buffer.
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferIdx(pub usize);

/// A [CompiledSchedule] is the output of the graph compiler.
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct CompiledSchedule<PortType: Enum> {
    /// A list of nodes, delays, and summing points to
    /// evaluate in order to render audio, in topological order.
    pub schedule: Vec<ScheduleEntry<PortType>>,
    /// A list of delays that were inserted into the graph.
    pub delays: Vec<InsertedDelay<PortType>>,
    /// The total number of buffers required to allocate, for
    /// each type of port.
    pub num_buffers: EnumMapArray<PortType, usize>,
}

/// A [ScheduleEntry] is one element of the schedule to evalute.
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum ScheduleEntry<PortType> {
    /// One of the input nodes, to process
    Node(ScheduledNode<PortType>),
    /// A delay that was inserted for latency compensation
    Delay(InsertedDelay<PortType>),
    /// A sum that was inserted to merge multiple inputs into
    /// the same port.
    Sum(InsertedSum<PortType>),
}

/// A [ScheduledNode] is a [Node] that has been assigned buffers
/// and a place in the schedule.
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct ScheduledNode<PortType> {
    /// The unique ID of this node.
    pub id: NodeID,
    /// The latency of this node. Kept for debugging and visualization.
    pub latency: f64,
    /// The assigned input buffers.
    pub input_buffers: Vec<BufferAssignment<PortType>>,
    /// The assigned output buffers.
    pub output_buffers: Vec<BufferAssignment<PortType>>,
}

impl<PortType: PartialEq> PartialEq for ScheduledNode<PortType> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.input_buffers == other.input_buffers
            && self.output_buffers == other.output_buffers
    }
}

impl<PortType: Eq> Eq for ScheduledNode<PortType> {}

/// An [InsertedDelay] represents a required delay node to be inserted
/// along some edge in order to compensate for different latencies along
/// paths of the graph.
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct InsertedDelay<PortType> {
    /// The edge that this delay corresponds to. Kept for debugging and visualization.
    pub edge: Edge,
    /// The amount of delay to apply to the input.
    pub delay: f64,
    /// The input data to read.
    pub input_buffer: BufferAssignment<PortType>,
    /// The output buffer to write delayed into to.
    pub output_buffer: BufferAssignment<PortType>,
}

/// An [InsertedSum] represents a point where multiple edges need to be merged
/// into a single buffer, in order to support multiple inputs into the same
/// port.
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsertedSum<PortType> {
    /// The input buffers that will be summed
    pub input_buffers: Vec<BufferAssignment<PortType>>,
    /// The output buffer to write to
    pub output_buffer: BufferAssignment<PortType>,
}

/// A [Buffer Assignment] represents a single buffer assigned to an input
/// or output port.
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct BufferAssignment<PortType> {
    /// The index of the buffer assigned
    pub buffer_index: BufferIdx,
    /// The index of the type of data in this buffer
    pub type_index: PortType,
    /// Whether the engine should clear the buffer before
    /// passing it to a process
    pub should_clear: bool,
    /// The ID of the port this buffer is mapped to
    pub port_id: PortID,
    /// Buffers are reused, the "generation" represnts
    /// how many times this buffer has been used before
    /// this assignment. Kept for debugging and visualization.
    pub generation: usize,
}

impl From<usize> for BufferIdx {
    fn from(i: usize) -> Self {
        BufferIdx(i)
    }
}
impl From<BufferIdx> for usize {
    fn from(i: BufferIdx) -> Self {
        i.0
    }
}
