//! Base graph implementation.
//!
//! Uses [`SlotMap`] as the backing storage for node and edge IDs (represented by [`NodeId`] and [`EdgeId`]).
use crate::{Edge, EdgeId, Graph, NodeId, OwnedGraph};
use slotmap::SlotMap;

/// "Default" owned graph
#[derive(Debug, Default, Clone)]
pub struct GraphBase {
    nodes: SlotMap<NodeId, ()>,
    edges: SlotMap<EdgeId, Edge>,
}

impl GraphBase {
    /// Create a new, empty [`GraphBase`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl Graph for GraphBase {
    fn get_edge(&self, id: EdgeId) -> Option<Edge> {
        self.edges.get(id).copied()
    }

    fn nodes(&self) -> impl '_ + Iterator<Item = NodeId> {
        self.nodes.keys()
    }

    fn edges(&self) -> impl '_ + Iterator<Item = EdgeId> {
        self.edges.keys()
    }

    fn has_edge(&self, id: EdgeId) -> bool {
        self.edges.contains_key(id)
    }

    fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    fn num_edges(&self) -> usize {
        self.edges.len()
    }
}

impl OwnedGraph for GraphBase {
    fn add_node(&mut self) -> NodeId {
        self.nodes.insert(())
    }

    fn add_edge(&mut self, from: NodeId, to: NodeId) -> EdgeId {
        self.edges.insert(Edge { from, to })
    }
}
