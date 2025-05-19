//! Module of traversal algorithms.
use crate::errors::CycleDetected;
use crate::{EdgeId, Graph, NodeId};
use std::collections::{HashSet, VecDeque};

/// Labels used to classify edges during graph traversal.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum EdgeLabel {
    /// Indicates an edge leading to a node that has already been visited.
    Back,
    /// Indicates an edge leading to a node that has not yet been visited.
    Front,
}

/// Performs a depth-first search (DFS) traversal of the graph starting
/// from a given node. This function visits nodes and edges in the graph,
/// calling the provided callback (`on_edge`) for each edge encountered during
/// the traversal. The callback receives the edge's ID and a label indicating
/// whether the edge is a "Back" edge (to a previously visited node) or a
/// "Front" edge (to a not-yet-visited node).
///
/// ### Arguments
///
/// - `graph`: A reference to a graph implementing the [`Graph`] trait.
/// - `node`: The starting node for the DFS traversal.
/// - `on_edge`: A callback function accepting an [`EdgeId`] and an [`EdgeLabel`].
///   It will be called for each edge encountered during the DFS traversal.
///
/// ### Example
///
/// ```
/// use clogbox_graph::{GraphBase, NodeId, OwnedGraph};
/// use clogbox_graph::algorithms::{EdgeLabel, dfs};
///
/// // Create a sample graph
/// let mut graph = GraphBase::new();
///
/// let node1 = graph.add_node();
/// let node2 = graph.add_node();
/// let node3 = graph.add_node();
///
/// let edge1 = graph.add_edge(node1, node2);
/// let edge2 = graph.add_edge(node2, node3);
/// let edge3 = graph.add_edge(node1, node3);
///
/// // Perform DFS traversal
/// let mut visited_edges = Vec::new();
/// dfs(&graph, node1, |edge, label| {
///     visited_edges.push((edge, label));
/// });
///
/// assert_eq!(
///     visited_edges,
///     vec![
///         (edge1, EdgeLabel::Front),
///         (edge3, EdgeLabel::Front),
///         (edge2, EdgeLabel::Back),
///     ]
/// );
/// ```
pub fn dfs(graph: &impl Graph, node: NodeId, mut on_edge: impl FnMut(EdgeId, EdgeLabel)) {
    let mut explored_nodes = HashSet::new();
    let mut stack = VecDeque::from_iter([node]);
    while let Some(node) = stack.pop_back() {
        if explored_nodes.contains(&node) {
            continue;
        }
        explored_nodes.insert(node);
        for edge in graph.outgoing(node) {
            let w = graph.get_edge(edge).unwrap().to;
            let label = if explored_nodes.contains(&w) {
                EdgeLabel::Back
            } else {
                EdgeLabel::Front
            };
            on_edge(edge, label);
            stack.push_back(w);
        }
    }
}

/// Performs a breadth-first search (BFS) traversal of a directed graph.
///
/// This function starts from the given `node` and traverses the graph layer by layer.
/// For each edge encountered, it calls the provided callback function `on_edge`,
/// which receives the edge's ID and a label (`EdgeLabel::Back` or `EdgeLabel::Front`).
///
/// ### Arguments
///
/// - `graph`: A reference to a graph implementing the [`Graph`] trait.
/// - `node`: The starting node for the BFS traversal.
/// - `on_edge`: A callback function accepting an [`EdgeId`] and an [`EdgeLabel`].
///
/// ### Example
///
/// ```
/// use clogbox_graph::{GraphBase, NodeId, EdgeId, OwnedGraph};
/// use clogbox_graph::algorithms::{EdgeLabel, bfs};
///
/// let mut graph = GraphBase::new();
///
/// let node1 = graph.add_node();
/// let node2 = graph.add_node();
/// let node3 = graph.add_node();
/// let node4 = graph.add_node();
/// let edge1 = graph.add_edge(node1, node2);
/// let edge2 = graph.add_edge(node1, node3);
/// let edge3 = graph.add_edge(node3, node1);
/// let edge4 = graph.add_edge(node4, node1);
///
/// let mut visited_edges = Vec::new();
/// bfs(&graph, node1, |edge, label| {
///     visited_edges.push((edge, label));
/// });
///
/// assert_eq!(
///     vec![(edge1, EdgeLabel::Front), (edge2, EdgeLabel::Front), (edge3, EdgeLabel::Back)],
///     visited_edges
/// );
/// ```
pub fn bfs(graph: &impl Graph, node: NodeId, mut on_edge: impl FnMut(EdgeId, EdgeLabel)) {
    let mut explored_nodes = HashSet::new();
    let mut queue = VecDeque::from_iter([node]);
    while let Some(node) = queue.pop_front() {
        if explored_nodes.contains(&node) {
            continue;
        }
        explored_nodes.insert(node);
        for edge in graph.outgoing(node) {
            let w = graph.get_edge(edge).unwrap().to;
            let label = if explored_nodes.contains(&w) {
                EdgeLabel::Back
            } else {
                queue.push_back(w);
                EdgeLabel::Front
            };
            on_edge(edge, label);
        }
    }
}

/// Performs a topological sort on a directed acyclic graph (DAG) starting from the given node.
/// Calls the `on_leaf` callback function for each leaf node encountered during the traversal.
///
/// # Arguments
///
/// - `graph`: A reference to a graph implementing the [`Graph`] trait.
/// - `start`: The starting node for the topological sort.
/// - `on_leaf`: A callback function that is invoked with a [`NodeId`] for each leaf node.
///
/// # Examples
///
/// ```
/// use clogbox_graph::{GraphBase, NodeId, OwnedGraph};
/// use clogbox_graph::algorithms::topological_sort;
/// use clogbox_graph::errors::CycleDetected;
///
/// let mut graph = GraphBase::new();
///
/// let node1 = graph.add_node();
/// let node2 = graph.add_node();
/// let node3 = graph.add_node();
/// let node4 = graph.add_node();
///
/// graph.add_edge(node1, node2);
/// graph.add_edge(node2, node3);
/// graph.add_edge(node2, node4);
///
/// let mut sorted = Vec::new();
///
/// let result = topological_sort(&graph, node1, |leaf| {
///     sorted.push(leaf);
/// });
///
/// assert!(result.is_ok());
/// assert_eq!(vec![node1, node2, node4, node3], sorted);
/// ```
///
pub fn topological_sort(
    graph: &impl Graph,
    start: NodeId,
    mut on_leaf: impl FnMut(NodeId),
) -> Result<(), CycleDetected> {
    let mut explored_nodes = HashSet::new();
    let mut stack = VecDeque::from_iter([start]);
    while let Some(node) = stack.pop_back() {
        if explored_nodes.contains(&node) {
            return Err(CycleDetected);
        }
        on_leaf(node);
        explored_nodes.insert(node);
        for edge in graph.outgoing(node) {
            let w = graph.get_edge(edge).unwrap().to;
            stack.push_back(w);
        }
    }
    Ok(())
}
