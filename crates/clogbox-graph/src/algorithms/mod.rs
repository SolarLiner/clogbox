//! Module of algorithms working on graphs.
pub mod path;
pub mod traversal;

use crate::{AdjacencyList, Edge, EdgeId, FromGraph, Graph, NodeId};
use slotmap::SecondaryMap;

pub use path::*;
pub use traversal::*;

/// Determines whether the given graph contains a cycle.
///
/// This function checks for the existence of a cycle in the input graph
/// by leveraging the `bellman_ford` algorithm.
///
/// ### Arguments
///
/// - `graph`: A reference to a graph implementing the [`Graph`] trait.
///
/// ### Returns
///
/// - `true` if no cycles are detected in the graph;
/// - `false` if a cycle exists in the graph.
///
/// ### Notes
///
/// The function iterates through all nodes in the graph, running the
/// `bellman_ford` function with a zero-weight edge cost function. If
/// the `bellman_ford` algorithm completes successfully for all nodes,
/// the graph is considered to be acyclic.
pub fn has_cycle(graph: &impl Graph) -> bool {
    graph.nodes().all(|n| bellman_ford(graph, n, |_| 0.0).is_ok())
}

/// Assigns a color to each node in the graph such that adjacent nodes have different colors.
///
/// This function implements a greedy graph coloring algorithm for assigning colors (represented
/// as integers) to the nodes of a graph. The color of each node is determined based on the
/// colors assigned to its neighbors, ensuring no two adjacent nodes have the same color.
///
/// ### Arguments
///
/// - `graph`: A reference to a graph implementing the [`Graph`] trait.
///
/// ### Returns
///
/// A [`SecondaryMap<NodeId, usize>`] where:
///
/// - The keys are the node IDs.
/// - The values are the assigned colors (as [`usize`] integers).
///
/// ### Notes
///
/// This algorithm does not necessarily find the optimal (minimum number of colors)
/// solution. Instead, it uses a simple greedy strategy and assigns the smallest possible
/// color to each node based on the colors of its neighbors.
pub fn color_nodes(graph: &impl Graph) -> SecondaryMap<NodeId, usize> {
    let mut out = SecondaryMap::new();
    let adj = AdjacencyList::from_graph(graph);
    let min_color = |out: &SecondaryMap<NodeId, usize>, node| {
        adj.neighbors(node)
            .filter_map(|n| out.get(n).copied())
            .min()
            .unwrap_or(0)
    };
    for node in graph.nodes() {
        out.insert(node, min_color(&out, node));
    }
    out
}

/// Assigns a color to each edge in the graph such that no two edges incident on the same node share the same color.
///
/// This function implements a greedy edge coloring algorithm for assigning colors (represented as integers)
/// to the edges of a graph. The algorithm ensures that all edges incident on the same node are assigned different colors.
///
/// # Arguments
///
/// - `graph`: A reference to a graph implementing the [`Graph`] trait.
///
/// # Returns
///
/// A [`SecondaryMap<EdgeId, usize>`] where:
/// - The keys are the edge IDs.
/// - The values are the assigned colors (as [`usize`] integers).
///
/// # Notes
///
/// - This algorithm does not necessarily compute the optimal (minimum number of colors) edge coloring.
///   Instead, it focuses on a fast and simple assignment.
/// - The result is dependent on the order of edges visited during the iteration.
pub fn color_edges(graph: &impl Graph) -> SecondaryMap<EdgeId, usize> {
    let mut out = SecondaryMap::new();
    let adj = AdjacencyList::from_graph(graph);
    let min_color = |out: &SecondaryMap<EdgeId, usize>, edge| {
        let Edge { from, to } = adj.edges[edge];
        adj.incoming(from)
            .chain(adj.outgoing(to))
            .filter_map(|e| out.get(e).copied())
            .min()
            .unwrap_or(0)
    };
    for edge in graph.edges() {
        out.insert(edge, min_color(&out, edge));
    }
    out
}
