//! Input data structures to the audio graph compiler.

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use slotmap::{new_key_type, SecondaryMap, SlotMap};
use crate::module::RawModule;

new_key_type! { pub struct NodeID; pub struct EdgeID; }

pub(crate) type NodeMap<PortType> = SlotMap<NodeID, Node<PortType>>;
pub(crate) type EdgeMap = SlotMap<EdgeID, Edge>;
pub(crate) type ModuleMap<T> = SecondaryMap<NodeID, Box<dyn RawModule<Sample=T>>>;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct PortID(pub(crate) u32);
/*
/// The input IR used by the audio graph compiler.
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct AudioGraphCompilerInput {
    /// A list of nodes in the graph.
    pub nodes: Vec<Node>,
    /// A list of edges in the graph.
    pub edges: Vec<Edge>,
    /// The number of different port types used by the graph.
    pub num_port_types: usize,
}
*/

/// A [Node] is a single process in the audio network.
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct Node<PortType> {
    /// A globally unique identifier of the node.
    pub id: NodeID,
    /// A list of input ports used by the node
    pub inputs: Vec<Port<PortType>>,
    /// A list of output ports used by the node.
    pub outputs: Vec<Port<PortType>>,
    /// The latency this node adds to data flowing through it.
    pub latency: f64,
}

/// A [Port] is a single point of input or output data
/// for a node.
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug)]
pub struct Port<Type> {
    /// The ID of this [Port] for this [Node].
    ///
    /// This does not need to be a globally unique identifier,
    /// just unique to the [Node] it belongs to.
    pub id: PortID,
    /// A unique identifier for the type of data this port handles,
    /// for example nodes may have audio and event ports.
    pub port_type: Type,
}

/// An [Edge] is a connection from source node and port to a
/// destination node and port.
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Edge {
    /// A globally unique identifier for this connection.
    pub id: EdgeID,
    /// The ID of the source node used by this edge.
    pub src_node: NodeID,
    /// The ID of the source port used by this edge.
    pub src_port: PortID,
    /// The ID of the destination node used by this edge.
    pub dst_node: NodeID,
    /// The ID of the destination port used by this edge.
    pub dst_port: PortID,
}
