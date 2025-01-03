use super::{Edge, EdgeID, NodeID, PortID};
use thiserror::Error;

/// An error occurred while attempting to add a port to the graph.
#[derive(Debug, Clone, Copy, Error)]
pub enum AddPortError {
    /// The given node was not found in the graph.
    #[error("Could not find node with ID {0:?}")]
    NodeNotFound(NodeID),
    /// A port with this ID already exists on this node.
    #[error("Could not add port: port with ID {1:?} already exists in node with ID {0:?}")]
    PortAlreadyExists(NodeID, PortID),
}

/// An error occurred while attempting to remove a port from the
/// graph.
#[derive(Debug, Clone, Copy, Error)]
pub enum RemovePortError {
    /// The given node was not found in the graph.
    #[error("Could not find node with ID {0:?}")]
    NodeNotFound(NodeID),
    /// The given port was not found in this node.
    #[error("Could not remove port: port with ID {1:?} was not found in node with ID {0:?}")]
    PortNotFound(NodeID, PortID),
}

/// An error occurred while attempting to add an edge to the graph.
#[derive(Debug, Clone, Error)]
pub enum AddEdgeError {
    /// The given source node was not found in the graph.
    #[error("Could not add edge: could not find source node with ID {0:?}")]
    SrcNodeNotFound(NodeID),
    /// The given destination node was not found in the graph.
    #[error("Could not add edge: could not find destination node with ID {0:?}")]
    DstNodeNotFound(NodeID),
    /// The given source port was not found in the graph.
    #[error("Could not add edge: could not find source port with ID {1:?} on node with ID {0:?}")]
    SrcPortNotFound(NodeID, PortID),
    /// The given destination port was not found in the graph.
    #[error(
        "Could not add edge: could not find destination port with ID {1:?} on node with ID {0:?}"
    )]
    DstPortNotFound(NodeID, PortID),
    /// The source port and the destination port have different
    /// type indexes.
    #[error("Could not add edge: source port {src_port:?} on node {src_node_id:?} is of a different type than destination port {dst_port:?} on node {dst_node_id:?}")]
    TypeMismatch {
        src_node_id: NodeID,
        src_port: PortID,
        dst_node_id: NodeID,
        dst_port: PortID,
    },
    /// The edge already exists in the graph.
    #[error("Could not add edge: edge {0:?} already exists in the graph")]
    EdgeAlreadyExists(Edge),
    /// This edge would have created a cycle in the graph.
    #[error("Could not add edge: cycle was detected")]
    CycleDetected,
}

/// An error occurred while attempting to compile the audio graph
/// into a schedule.
#[derive(Debug, Clone, Copy, Error)]
pub enum CompileGraphError {
    /// A cycle was detected in the graph.
    #[error("Failed to compile audio graph: a cycle was detected")]
    CycleDetected,
    /// The input data contained an edge referring to a non-existing node.
    #[error("Failed to compile audio graph: input data contains an edge {0:?} referring to a non-existing node {1:?}")]
    NodeOnEdgeNotFound(Edge, NodeID),
    /// The input data contained multiple nodes with the same ID.
    #[error(
        "Failed to compile audio graph: input data contains multiple nodes with the same ID {0:?}"
    )]
    NodeIDNotUnique(NodeID),
    /// The input data contained multiple edges with the same ID.
    #[error(
        "Failed to compile audio graph: input data contains multiple edges with the same ID {0:?}"
    )]
    EdgeIDNotUnique(EdgeID),
    /// The input data contained a port with an out-of-bounds type index.
    #[error("Failed to compile audio graph: input data contains a port {1:?} on node {0:?} with a type index that is out of bounds for a graph with {2} types")]
    PortTypeIndexOutOfBounds(NodeID, PortID, usize),
}
