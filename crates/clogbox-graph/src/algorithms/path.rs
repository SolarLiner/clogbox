//! Module of shortest path algorithms.
use crate::errors::CycleDetected;
use crate::{AdjacencyList, Edge, EdgeId, FromGraph, Graph, NodeId};
use slotmap::SecondaryMap;
use std::collections::HashMap;

/// Finds the shortest path from a starting node to all other nodes in a graph
/// using the Bellman-Ford algorithm.
///
/// This algorithm computes the shortest path distances from a starting node `start`
/// to every other node in the graph. It also detects negative weight cycles in the graph.
///
/// ### Arguments
///
/// - `graph`: A reference to a graph implementing the [`Graph`] trait.
/// - `start`: The starting node for the shortest path computation.
/// - `edge_weight`: A closure that returns the weight of a given edge, where
///   the weight is represented as a `f32`.
///
/// ### Returns
///
/// A [`Result`] containing:
///
/// - `Ok(SecondaryMap<NodeId, f32>)`: A map where the keys are node IDs and
///   the values are the shortest path distances from the starting node.
/// - `Err(CycleDetected)`: If a negative weight cycle is detected during
///   the computation.
///
/// ### Errors
///
/// - Returns a [`CycleDetected`] error if a negative weight cycle is found
///   in the graph.
///
/// ### Notes
///
/// The Bellman-Ford algorithm is suitable for graphs that may have edges with
/// negative weights. However, if the graph does not contain negative weight edges,
/// other algorithms like Dijkstra's algorithm may perform better.
///
/// ### Example
///
/// ```rust
/// use clogbox_graph::{Graph, GraphBase, NodeId, OwnedGraph};
/// use clogbox_graph::errors::CycleDetected;
/// use clogbox_graph::algorithms::bellman_ford;
/// use slotmap::SecondaryMap;
///
/// let mut graph = GraphBase::new();
/// let a = graph.add_node();
/// let b = graph.add_node();
/// let c = graph.add_node();
/// let d = graph.add_node();
/// graph.add_edge(a, b);
/// graph.add_edge(a, c);
/// graph.add_edge(b, c);
/// graph.add_edge(b, d);
/// graph.add_edge(d, a); // Creates a cycle, but with a positive weight, so is considered OK for Bellman-Ford.
///
/// let edge_weight = |edge| 1.0; // Assume all edges have a weight of 1.0.
///
/// let expected_distances = SecondaryMap::from_iter([
///     (a, 0.0),
///     (b, 1.0),
///     (c, 1.0),
///     (d, 2.0),
/// ]);
/// assert_eq!(Ok(expected_distances), bellman_ford(&graph, a, edge_weight));
/// ```
pub fn bellman_ford(
    graph: &impl Graph,
    start: NodeId,
    edge_weight: impl Fn(EdgeId) -> f32,
) -> Result<SecondaryMap<NodeId, f32>, CycleDetected> {
    let adj = AdjacencyList::from_graph(graph);
    let mut distances = SecondaryMap::from_iter(adj.nodes().map(|id| (id, f32::INFINITY)));
    distances.insert(start, 0.0);
    let n = adj.num_nodes();
    for i in 0..n {
        for edge in adj.edges() {
            let w = edge_weight(edge);
            let Edge { from, to } = adj.get_edge(edge).unwrap();
            if distances[from] + w < distances[to] {
                if i == n - 1 {
                    return Err(CycleDetected);
                }
                distances.insert(to, distances[from] + w);
            }
        }
    }

    Ok(distances)
}

/// Computes the shortest distances between all pairs of nodes in the graph using the Floyd-Warshall algorithm.
///
/// This algorithm calculates the shortest paths between all pairs of nodes in a graph.
/// It is suitable for dense graphs and allows for edges with negative weights, as long as
/// there are no negative weight cycles.
///
/// ### Arguments
///
/// - `graph`: A reference to a graph implementing the [`Graph`] trait.
/// - `edge_weight`: A closure that returns the weight of a given edge, represented as a `f32`.
///
/// ### Returns
///
/// A [`HashMap<(NodeId, NodeId), f32>`] where:
/// - The keys are pairs of node IDs representing all node-to-node relationships.
/// - The values are the shortest path distances between the corresponding nodes.
///
/// ### Notes
///
/// The Floyd-Warshall algorithm is most efficient for dense graphs with a large number of edges.
/// For sparse graphs, other algorithms like Dijkstra or Bellman-Ford might be more efficient.
///
/// ### Example
///
/// ```rust
/// use clogbox_graph::{Graph, GraphBase, NodeId, OwnedGraph};
/// use clogbox_graph::algorithms::floyd_warshall;
/// use std::collections::HashMap;
///
/// let mut graph = GraphBase::new();
/// let a = graph.add_node();
/// let b = graph.add_node();
/// let c = graph.add_node();
/// graph.add_edge(a, b);
/// graph.add_edge(b, c);
/// graph.add_edge(a, c);
///
/// let edge_weight = |edge| 1.0; // Assume all edges have a weight of 1.0.
///
/// let distances = floyd_warshall(&graph, edge_weight);
///
/// let expected_distances = HashMap::from([
///     ((a, a), 0.0),
///     ((a, b), 1.0),
///     ((a, c), 1.0),
///     ((b, b), 0.0),
///     ((b, c), 1.0),
///     ((b, a), f32::INFINITY),
///     ((c, c), 0.0),
///     ((c, a), f32::INFINITY),
///     ((c, b), f32::INFINITY),
/// ]);
///
/// for ((from, to), &expected) in &expected_distances {
///     let actual = distances.get(&(*from, *to)).copied().unwrap_or(f32::INFINITY);
///     if expected.is_infinite() {
///        assert!(actual.is_infinite());
///     } else {
///         assert!((actual - expected).abs() < 1e-6, "Mismatch for {:?} -> {:?}: expected {}, got {}", from, to, expected, actual);
///     }
/// }
/// ```
pub fn floyd_warshall(graph: &impl Graph, edge_weight: impl Fn(EdgeId) -> f32) -> HashMap<(NodeId, NodeId), f32> {
    let adj = AdjacencyList::from_graph(graph);
    let mut distances = HashMap::from_iter(
        adj.edges()
            .map(|id| {
                let edge = adj.get_edge(id).unwrap();
                ((edge.from, edge.to), edge_weight(id))
            })
            .chain(adj.nodes().map(|id| ((id, id), 0.0))),
    );
    for k in adj.nodes() {
        for i in adj.nodes() {
            for j in adj.nodes() {
                let ik = distances.get(&(i, k)).copied().unwrap_or(f32::INFINITY);
                let kj = distances.get(&(k, j)).copied().unwrap_or(f32::INFINITY);
                distances
                    .entry((i, j))
                    .and_modify(|dist| {
                        *dist = dist.min(ik + kj);
                    })
                    .or_insert(ik + kj);
            }
        }
    }

    distances
}
