#![warn(missing_docs)]
//! # Graph datastructures
//!
//! Implements graph datastructures and algorithms working with graphs.
//!
//! The datastructures only use IDs to track nodes and edges; it is the responsibility of the
//! user to store their own data. When needed, the algorithms will have callbacks as arguments to
//! provide relevant data (i.e. weight for shortest path algorithms).
use slotmap::new_key_type;
use std::collections::HashSet;
use std::ops;

pub mod adjacency;
pub mod algorithms;
pub mod base;
pub mod data;
pub mod errors;
mod wrappers;

pub use adjacency::AdjacencyList;
pub use base::GraphBase;

new_key_type! {
    /// Type of node IDs.
    pub struct NodeId;
    /// Type of edge IDs.
    pub struct EdgeId;
}

/// Edge data, connecting two nodes together.
#[derive(Debug, Copy, Clone)]
pub struct Edge {
    /// Source edge
    pub from: NodeId,
    /// Target edge
    pub to: NodeId,
}

/// A trait representing a graph structure with basic graph operations.
///
/// This trait defines the minimal set of operations to interact with a graph,
/// including retrieving nodes, edges, and checking relationships between them.
///
/// # Examples
///
/// ```
/// use clogbox_graph::{Graph, GraphBase, NodeId, OwnedGraph};
///
/// let mut graph = GraphBase::new();
/// let node1 = graph.add_node();
/// let node2 = graph.add_node();
/// let edge = graph.add_edge(node1, node2);
///
/// assert!(graph.has_node(node1));
/// assert!(graph.has_edge(edge));
/// assert!(graph.has_edge_between(node1, node2));
/// ```
pub trait Graph {
    /// Returns the edge data (source and target node id) for this id, if it exists.
    ///
    /// # Arguments
    ///
    /// - `id`: Edge ID
    fn get_edge(&self, id: EdgeId) -> Option<Edge>;

    /// Returns an iterator over all nodes in the graph.
    fn nodes(&self) -> impl '_ + Iterator<Item = NodeId>;

    /// Returns an iterator over all edges in the graph.
    fn edges(&self) -> impl '_ + Iterator<Item = EdgeId>;

    /// Checks whether the graph contains a specific node.
    ///
    /// This method tests if a node with the given [`NodeId`] exists in the graph
    /// by iterating through all nodes and comparing their IDs.
    ///
    /// # Arguments
    ///
    /// * `node` - The [`NodeId`] to check for existence in the graph.
    ///
    /// # Returns
    ///
    /// Returns `true` if the node exists in the graph, otherwise returns `false`.
    fn has_node(&self, node: NodeId) -> bool {
        self.nodes().any(|id| id == node)
    }

    /// Checks whether the graph contains an edge with a specific [`EdgeId`].
    ///
    /// # Arguments
    ///
    /// * `id` - The [`EdgeId`] to check for existence.
    fn has_edge(&self, id: EdgeId) -> bool {
        self.get_edge(id).is_some()
    }

    /// Returns the total number of nodes in the graph.
    fn num_nodes(&self) -> usize {
        self.nodes().count()
    }

    /// Returns the total number of edges in the graph.
    fn num_edges(&self) -> usize {
        self.edges().count()
    }

    /// Returns true if the nodes are directly connected to each other in the graph.
    fn has_edge_between(&self, from: NodeId, to: NodeId) -> bool {
        self.edges()
            .map(|id| self.get_edge(id).unwrap())
            .any(|e| e.from == from && e.to == to)
    }

    /// Returns an iterator over all edges between two nodes in the graph.
    ///
    /// This function finds all edges that start from the provided `from` node
    /// and end at the `to` node.
    ///
    /// # Arguments
    ///
    /// * `from` - The [`NodeId`] of the source node.
    /// * `to` - The [`NodeId`] of the target node.
    ///
    /// # Returns
    ///
    /// An iterator over all [`EdgeId`]s that connect the given nodes.
    ///
    /// # Panics
    ///
    /// Panics if either the `from` or `to` node does not exist in the graph.
    fn edges_between(&self, from: NodeId, to: NodeId) -> impl '_ + Iterator<Item = EdgeId> {
        self.outgoing(from)
            .filter(move |id| self.get_edge(*id).unwrap().to == to)
    }

    /// Returns an iterator over all incoming edges for a given node.
    ///
    /// # Arguments
    ///
    /// * `node` - The [`NodeId`] for which incoming edges are retrieved.
    ///
    /// # Panics
    ///
    /// Panics if the [`NodeId`] does not exist in the graph.
    fn incoming(&self, node: NodeId) -> impl '_ + Iterator<Item = EdgeId> {
        self.edges().filter(move |id| self.get_edge(*id).unwrap().to == node)
    }

    /// Returns an iterator over all outgoing edges for a given node.
    ///
    /// # Arguments
    ///
    /// * `node` - The [`NodeId`] for which outgoing edges are retrieved.
    ///
    /// # Panics
    ///
    /// Panics if the [`NodeId`] does not exist in the graph.
    fn outgoing(&self, node: NodeId) -> impl '_ + Iterator<Item = EdgeId> {
        self.edges().filter(move |id| self.get_edge(*id).unwrap().from == node)
    }

    /// Returns an iterator over all unique neighbors of a given node.
    ///
    /// A neighbor is defined as any node directly connected to the given node
    /// by an edge, either as the source or the target of that edge.
    ///
    /// This method ensures that each neighbor appears only once in the result,
    /// even if multiple edges connect the nodes.
    ///
    /// # Arguments
    ///
    /// * `node` - The [`NodeId`] of the node whose neighbors are being retrieved.
    ///
    /// # Returns
    ///
    /// An iterator over all [`NodeId`]s that are neighbors of the given node.
    ///
    /// # Panics
    ///
    /// Panics if the given [`NodeId`] does not exist in the graph.
    fn neighbors(&self, node: NodeId) -> impl '_ + Iterator<Item = NodeId> {
        let mut set = HashSet::new();
        self.edges()
            .filter_map(move |id| {
                let e = self.get_edge(id).unwrap();
                (e.from == node)
                    .then_some(e.to)
                    .or_else(|| (e.to == node).then_some(e.from))
            })
            .filter(move |n| {
                let unique = !set.contains(n);
                set.insert(*n);
                unique
            })
    }
}

/// A trait representing a graph that owns its nodes and edges.
///
/// *Owning* here means that the graph generates the IDs. For the cases where you already
/// have IDs and are looking to reuse them, use [`SecondaryGraph`].
///
/// # Examples
///
/// ```
/// use clogbox_graph::{Graph, NodeId};
/// use clogbox_graph::GraphBase;
/// use clogbox_graph::OwnedGraph;
///
/// let mut graph = GraphBase::new();
/// let node1 = graph.add_node();
/// let node2 = graph.add_node();
///
/// // Adds an edge from `node1` to `node2`.
/// let edge = graph.add_edge(node1, node2);
///
/// assert!(graph.has_edge(edge));
/// ```
pub trait OwnedGraph: Graph {
    /// Adds a new node into the graph.
    fn add_node(&mut self) -> NodeId;

    /// Adds an edge to the graph.
    ///
    /// # Arguments
    ///
    /// - `from`: Source [`NodeId`].
    /// - `to`  : Target [`NodeId`].
    fn add_edge(&mut self, from: NodeId, to: NodeId) -> EdgeId;
}

/// A trait representing a secondary graph structure.
///
/// This trait allows operations on graphs where the identifiers for nodes and edges
/// are reused or externally provided. Unlike [`OwnedGraph`], it assumes that the
/// graph does not generate IDs on its own but instead operates on existing IDs.
///
/// This is useful for synchronizing changes in a secondary data structure with another graph.
///
/// # Examples
///
/// ```
/// use clogbox_graph::{Graph, NodeId, OwnedGraph, SecondaryGraph, Edge, AdjacencyList};
/// use clogbox_graph::base::GraphBase;
///
/// let mut owning_graph = GraphBase::new();
/// let mut secondary_graph = AdjacencyList::new();
///
/// let node1 = owning_graph.add_node();
/// let node2 = owning_graph.add_node();
/// let edge1 = owning_graph.add_edge(node1, node2);
///
/// secondary_graph.add_node(node1);
/// secondary_graph.add_node(node2);
/// secondary_graph.add_edge(edge1, Edge { from: node1, to: node2 });
///
/// assert!(owning_graph.has_edge(edge1));
/// ```
pub trait SecondaryGraph: Graph {
    /// Adds a node to the secondary graph using an existing [`NodeId`].
    ///
    /// This method expects that the node ID is already known,
    /// as no new ID will be created.
    ///
    /// # Arguments
    ///
    /// * `id` - The [`NodeId`] of the node to add.
    ///
    /// # Panics
    ///
    /// Panics if a node with the same ID already exists in the secondary graph.
    fn add_node(&mut self, id: NodeId);

    /// Adds an edge to the secondary graph using an existing [`EdgeId`] and [`Edge`] data.
    ///
    /// This method expects that the edge ID and associated edge data are already known.
    ///
    /// # Arguments
    ///
    /// * `id` - The [`EdgeId`] of the edge to add.
    /// * `edge` - The [`Edge`] data describing the connection between two nodes.
    ///
    /// # Panics
    ///
    /// Panics if an edge with the same ID already exists in the secondary graph.
    fn add_edge(&mut self, id: EdgeId, edge: Edge);
}

/// A trait for constructing a type from an existing [`Graph`] structure.
///
/// This trait defines a method for creating an instance of a type
/// by utilizing or transforming data from an existing graph. It can
/// be particularly useful when converting or adapting between different
/// graph representations.
///
/// # Examples
///
/// ```
/// use clogbox_graph::{Graph, GraphBase, FromGraph, OwnedGraph};
/// use clogbox_graph::adjacency::AdjacencyList;
///
/// let mut base_graph = GraphBase::new();
/// let node1 = base_graph.add_node();
/// let node2 = base_graph.add_node();
/// base_graph.add_edge(node1, node2);
///
/// let adjacency_list = AdjacencyList::from_graph(&base_graph);
/// assert_eq!(adjacency_list.num_nodes(), base_graph.num_nodes());
/// assert_eq!(adjacency_list.num_edges(), base_graph.num_edges());
/// ```
pub trait FromGraph: Sized {
    /// Constructs an instance of a type by using an existing [`Graph`] structure.
    ///
    /// This method accepts any type that implements the [`Graph`] trait and uses its data
    /// (such as nodes and edges) to construct a new representation.
    ///
    /// # Arguments
    ///
    /// * `graph` - A reference to an existing graph that will be used for constructing the new type.
    ///
    /// # Returns
    ///
    /// An instance of the implementing type that is constructed based on the provided graph.
    fn from_graph(graph: &impl Graph) -> Self;
}

/// Type which wraps both an owning graph type and a secondary graph, to keep them in sync.
///
/// This is also a way to augment any [`SecondaryGraph`] and make it [`OwnedGraph`].
pub struct Attached<OG, SG> {
    /// Owning graph. All IDs are generated here. This is also the graph used as source of truth in
    /// the [`Graph`] methods.
    pub owning: OG,
    /// Secondary graph. Kept in sync with the owning graph, but not used outside of this.
    pub secondary: SG,
}

impl<OG: ops::Index<EdgeId, Output = Edge>, SG> ops::Index<EdgeId> for Attached<OG, SG> {
    type Output = Edge;

    fn index(&self, index: EdgeId) -> &Self::Output {
        &self.owning[index]
    }
}

impl<OS: Graph, SG> Graph for Attached<OS, SG> {
    fn get_edge(&self, id: EdgeId) -> Option<Edge> {
        self.owning.get_edge(id)
    }

    fn nodes(&self) -> impl '_ + Iterator<Item = NodeId> {
        self.owning.nodes()
    }

    fn edges(&self) -> impl '_ + Iterator<Item = EdgeId> {
        self.owning.edges()
    }
}

impl<OS: OwnedGraph, SG: SecondaryGraph> OwnedGraph for Attached<OS, SG> {
    fn add_node(&mut self) -> NodeId {
        let id = self.owning.add_node();
        self.secondary.add_node(id);
        id
    }

    fn add_edge(&mut self, from: NodeId, to: NodeId) -> EdgeId {
        let id = self.owning.add_edge(from, to);
        self.secondary.add_edge(id, Edge { from, to });
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::GraphBase;

    #[test]
    fn test_has_edge() {
        let mut graph = GraphBase::new();
        let node1 = graph.add_node();
        let node2 = graph.add_node();
        graph.add_edge(node1, node2);

        assert!(graph.has_edge_between(node1, node2));
        assert!(!graph.has_edge_between(node2, node1));
    }

    #[test]
    fn test_incoming_edges() {
        let mut graph = GraphBase::new();
        let node1 = graph.add_node();
        let node2 = graph.add_node();
        let edge_id = graph.add_edge(node2, node1);

        let incoming: Vec<EdgeId> = graph.incoming(node1).collect();
        assert_eq!(incoming, vec![edge_id]);
    }

    #[test]
    fn test_outgoing_edges() {
        let mut graph = GraphBase::new();
        let node1 = graph.add_node();
        let node2 = graph.add_node();
        let edge_id = graph.add_edge(node1, node2);

        let outgoing: Vec<EdgeId> = graph.outgoing(node1).collect();
        assert_eq!(outgoing, vec![edge_id]);
    }

    #[test]
    fn test_neighbors() {
        let mut graph = GraphBase::new();
        let node1 = graph.add_node();
        let node2 = graph.add_node();
        let node3 = graph.add_node();
        graph.add_edge(node1, node2);
        graph.add_edge(node2, node1);
        graph.add_edge(node1, node3);

        let neighbors: HashSet<NodeId> = graph.neighbors(node1).collect();
        assert_eq!(neighbors, HashSet::from([node2, node3]));
    }
}
