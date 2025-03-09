//! A module providing an adjacency list representation of a graph.
//!
//! This module defines the [`AdjacencyList`] struct and its implementation,
//! supporting basic graph operations such as traversing nodes and edges,
//! accessing incoming and outgoing edges, and constructing adjacency lists
//! from existing graphs.
use crate::{Edge, EdgeId, FromGraph, Graph, NodeId, SecondaryGraph};
use slotmap::SecondaryMap;

/// A graph represented as an adjacency list.
///
/// The [`AdjacencyList`] struct implements the [`Graph`], [`SecondaryGraph`], and [`FromGraph`] traits
/// to provide a rich set of graph-related operations. It maintains internal mappings for nodes,
/// edges, incoming edges, and outgoing edges.
#[derive(Debug, Clone, Default)]
pub struct AdjacencyList {
    /// A mapping of nodes in the graph to a placeholder value.
    ///
    /// This map tracks all nodes that belong to the graph.
    pub nodes: SecondaryMap<NodeId, ()>,
    /// A mapping of edges in the graph to their associated [`Edge`] data.
    ///
    /// This map stores all edges within the graph.
    pub edges: SecondaryMap<EdgeId, Edge>,
    /// A mapping of each node to its incoming edges.
    ///
    /// For any [`NodeId`], this map contains the [`EdgeId`]s of all edges
    /// that terminate at the node.
    pub incoming: SecondaryMap<NodeId, Vec<EdgeId>>,
    /// A mapping of each node to its outgoing edges.
    ///
    /// For any [`NodeId`], this map contains the [`EdgeId`]s of all edges
    /// that originate from the node.
    pub outgoing: SecondaryMap<NodeId, Vec<EdgeId>>,
}

impl AdjacencyList {
    /// Create a new, empty adjacency list graph.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Graph for AdjacencyList {
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

    fn incoming(&self, node: NodeId) -> impl '_ + Iterator<Item = EdgeId> {
        self.incoming[node].iter().copied()
    }

    fn outgoing(&self, node: NodeId) -> impl '_ + Iterator<Item = EdgeId> {
        self.outgoing[node].iter().copied()
    }
}

impl SecondaryGraph for AdjacencyList {
    fn add_node(&mut self, id: NodeId) {
        self.nodes.insert(id, ());
    }

    fn add_edge(&mut self, id: EdgeId, edge: Edge) {
        self.edges.insert(id, edge);
    }
}

impl FromGraph for AdjacencyList {
    fn from_graph(graph: &impl Graph) -> Self {
        let incoming = SecondaryMap::from_iter(graph.nodes().map(|node| (node, graph.incoming(node).collect())));
        let outgoing = SecondaryMap::from_iter(graph.nodes().map(|node| (node, graph.outgoing(node).collect())));
        Self {
            nodes: SecondaryMap::from_iter(graph.nodes().map(|n| (n, ()))),
            edges: SecondaryMap::from_iter(graph.edges().map(|id| (id, graph.get_edge(id).unwrap()))),
            incoming,
            outgoing,
        }
    }
}
