use crate::{Edge, EdgeId, Graph, NodeId, OwnedGraph, SecondaryGraph};

/// Type which reverses the direction of all edges of the inner graph.
#[derive(Debug, Clone)]
pub struct Reversed<G>(pub G);

impl<G: Graph> Graph for Reversed<G> {
    fn get_edge(&self, id: EdgeId) -> Option<Edge> {
        self.0.get_edge(id).map(|e| Edge { from: e.to, to: e.from })
    }
    fn nodes(&self) -> impl '_ + Iterator<Item = NodeId> {
        self.0.nodes()
    }

    fn edges(&self) -> impl '_ + Iterator<Item = EdgeId> {
        self.0.edges()
    }

    fn has_edge_between(&self, from: NodeId, to: NodeId) -> bool {
        self.0.has_edge_between(to, from)
    }

    fn edges_between(&self, from: NodeId, to: NodeId) -> impl '_ + Iterator<Item = EdgeId> {
        self.0.edges_between(to, from)
    }

    fn incoming(&self, node: NodeId) -> impl '_ + Iterator<Item = EdgeId> {
        self.0.outgoing(node)
    }

    fn outgoing(&self, node: NodeId) -> impl '_ + Iterator<Item = EdgeId> {
        self.0.incoming(node)
    }
}

impl<G: OwnedGraph> OwnedGraph for Reversed<G> {
    fn add_node(&mut self) -> NodeId {
        self.0.add_node()
    }

    fn add_edge(&mut self, from: NodeId, to: NodeId) -> EdgeId {
        self.0.add_edge(to, from)
    }
}

impl<G: SecondaryGraph> SecondaryGraph for Reversed<G> {
    fn add_node(&mut self, id: NodeId) {
        self.0.add_node(id)
    }

    fn add_edge(&mut self, id: EdgeId, edge: Edge) {
        self.0.add_edge(
            id,
            Edge {
                from: edge.to,
                to: edge.from,
            },
        )
    }
}
