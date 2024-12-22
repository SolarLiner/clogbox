//! Structs that help construct and modify audio graphs.

use super::error::{AddEdgeError, AddPortError, CompileGraphError, RemovePortError};
use super::{
    CompiledSchedule, Edge, EdgeID, EdgeMap, GraphIR, Node, NodeID, NodeMap, Port, PortID,
};
use clogbox_enum::Enum;
use slotmap::new_key_type;
use std::marker::PhantomData;

new_key_type! { struct NodeKey; struct EdgeKey; }

/// A helper struct to construct and modify audio graphs.
pub struct AudioGraphHelper<PortType> {
    nodes: NodeMap<PortType>,
    edges: EdgeMap,
    needs_compile: bool,
    __port_type: PhantomData<PortType>,
}

impl<PortType: Enum> AudioGraphHelper<PortType> {
    /// Construct a new [AudioGraphHelper].
    ///
    /// * `num_port_types` - The total number of port types that can
    /// exist in this audio graph. For example, if your graph can have
    /// an audio port type and an event port type, then this should be
    /// `2`. Ports of different types cannot be connected together.
    ///
    /// ## Panics
    ///
    /// This will panic if `num_port_types == 0`.
    pub fn new() -> Self {
        Self {
            nodes: NodeMap::default(),
            edges: EdgeMap::default(),
            needs_compile: false,
            __port_type: PhantomData,
        }
    }

    /// Add a new [Node] to the audio graph.
    ///
    /// This will return the globally unique ID assigned to this node.
    pub fn add_node(&mut self, latency: f64) -> NodeID {
        let new_id = self.nodes.insert_with_key(|id| Node {
            id,
            inputs: vec![],
            outputs: vec![],
            latency,
        });

        self.needs_compile = true;
        new_id
    }

    /// Get info about a node.
    ///
    /// This will return `None` if a node with the given ID does not
    /// exist in the graph.
    pub fn node(&self, node_id: NodeID) -> &Node<PortType> {
        &self.nodes[node_id]
    }

    /// Set the latency of the given [Node] in the audio graph.
    ///
    /// This will return an error if a node with the given ID does not
    /// exist in the graph.
    pub fn set_node_latency(&mut self, node_id: NodeID, latency: f64) -> Result<(), ()> {
        let node = &mut self.nodes[node_id];

        if node.latency != latency {
            node.latency = latency;
            self.needs_compile = true;
        }

        Ok(())
    }

    /// Remove the given node from the graph.
    ///
    /// This will automatically remove all edges from the graph that
    /// were connected to this node.
    ///
    /// On success, this returns a list of all edges that were removed
    /// from the graph as a result of removing this node.
    ///
    /// This will return an error if a node with the given ID does not
    /// exist in the graph.
    pub fn remove_node(&mut self, node_id: NodeID) -> Result<Vec<EdgeID>, ()> {
        let Some(node) = self.nodes.remove(node_id) else {
            return Err(());
        };

        let mut removed_edges: Vec<EdgeID> = Vec::new();

        for port in node.inputs.iter().chain(node.outputs.iter()) {
            removed_edges.append(&mut self.remove_edges_with_port(node_id, port.id));
        }

        self.needs_compile = true;

        Ok(removed_edges)
    }

    /// Get a list of all the existing nodes in the graph.
    pub fn nodes<'a>(&'a self) -> impl Iterator<Item = &'a Node<PortType>> + 'a {
        self.nodes.values()
    }

    /// Get a list of all the existing edges in the graph.
    pub fn edges<'a>(&'a self) -> impl Iterator<Item = &'a Edge> + 'a {
        self.edges.values()
    }

    /// Add a new [Port] to the graph.
    ///
    /// * `node_id` - The ID of the [Node] this port will be added to.
    /// * `port_id` - The identifier for this port. This does not need
    /// to be a globally unique identifier, just unique to the [Node]
    /// it belongs to.
    /// * `type_idx` - The type index of this port. This must be less
    /// than the value of `num_port_types` set in the constructor
    /// of this helper struct. Ports of different types cannot be
    /// connected to eachother.
    /// * `is_input` - `true` if this is an input port, `false` if this
    /// is an output port.
    ///
    /// If this returns an error, then the audio graph has not been
    /// modified.
    pub fn add_port(
        &mut self,
        node_id: NodeID,
        port_id: PortID,
        port_type: PortType,
        is_input: bool,
    ) -> Result<(), AddPortError> {
        let node = &mut self.nodes[node_id];

        let new_port = Port {
            id: port_id,
            port_type,
        };

        for p in node.inputs.iter().chain(node.outputs.iter()) {
            if p.id == port_id {
                return Err(AddPortError::PortAlreadyExists(node_id, port_id));
            }
        }

        if is_input {
            node.inputs.push(new_port);
        } else {
            node.outputs.push(new_port);
        }

        self.needs_compile = true;

        Ok(())
    }

    /// Remove the given port from the graph.
    ///
    /// This will automatically remove all edges from the graph that
    /// were connected to this port.
    ///
    /// * `node_id` - The ID of the node which the port belongs to.
    /// * `port_id` - The ID of the port to remove.
    ///
    /// On success, this returns a list of all edges that were removed
    /// from the graph as a result of removing this node.
    ///
    /// If this returns an error, then the audio graph has not been
    /// modified.
    pub fn remove_port(
        &mut self,
        node_id: NodeID,
        port_id: PortID,
    ) -> Result<Vec<EdgeID>, RemovePortError> {
        let node = &mut self.nodes[node_id];

        if let Some(i) = node.inputs.iter().position(|p| p.id == port_id) {
            node.inputs.remove(i);
        } else {
            if let Some(i) = node.outputs.iter().position(|p| p.id == port_id) {
                node.outputs.remove(i);
            } else {
                return Err(RemovePortError::PortNotFound(node_id, port_id));
            }
        }

        self.needs_compile = true;

        Ok(self.remove_edges_with_port(node_id, port_id))
    }

    /// Add an [Edge] (port connection) to the graph.
    ///
    /// * `src_node_id` - The ID of the source node.
    /// * `src_port_id` - The ID of the source port. This must be an output
    /// port on the source node.
    /// * `dst_node_id` - The ID of the destination node.
    /// * `dst_port_id` - The ID of the destination port. This must be an
    /// input port on the destination node.
    /// * `check_for_cycles` - If `true`, then this will run a check to
    /// see if adding this edge will create a cycle in the graph, and
    /// return an error if it does.
    ///     * Only set this to `false` if you are certain that adding this
    ///     edge won't create a cyle, such as when restoring a previously
    ///     valid graph from a save state.
    ///
    /// If successful, this returns the globally unique identifier assigned
    /// to this edge.
    ///
    /// If this returns an error, then the audio graph has not been
    /// modified.
    pub fn add_edge(
        &mut self,
        src_node_id: NodeID,
        src_port_id: PortID,
        dst_node_id: NodeID,
        dst_port_id: PortID,
        check_for_cycles: bool,
    ) -> Result<EdgeID, AddEdgeError> {
        let src_port = {
            self.nodes[src_node_id]
                .outputs
                .iter()
                .find(|p| p.id == src_port_id)
                .ok_or(AddEdgeError::SrcPortNotFound(src_node_id, src_port_id))
        }?;
        let dst_port = {
            self.nodes[dst_node_id]
                .inputs
                .iter()
                .find(|p| p.id == dst_port_id)
                .ok_or(AddEdgeError::DstPortNotFound(dst_node_id, dst_port_id))
        }?;

        if src_port.port_type != dst_port.port_type {
            return Err(AddEdgeError::TypeMismatch {
                src_node_id,
                src_port: src_port_id,
                dst_node_id,
                dst_port: dst_port_id,
            });
        }

        for e in self.edges.values() {
            if e.src_node == src_node_id
                && e.dst_node == dst_node_id
                && e.src_port == src_port_id
                && e.dst_port == dst_port_id
            {
                return Err(AddEdgeError::EdgeAlreadyExists(*e));
            }
        }

        if src_node_id == dst_node_id {
            return Err(AddEdgeError::CycleDetected);
        }
        let new_edge_id = self.edges.insert_with_key(|id| Edge {
            id,
            src_node: src_node_id,
            src_port: src_port_id,
            dst_node: dst_node_id,
            dst_port: dst_port_id,
        });

        if check_for_cycles {
            if self.cycle_detected() {
                self.edges.remove(new_edge_id);

                return Err(AddEdgeError::CycleDetected);
            }
        }

        self.needs_compile = true;

        Ok(new_edge_id)
    }

    /// Remove the given [Edge] (port connection) from the graph.
    ///
    /// This will return an error if the given edge does not exist in
    /// the graph. In this case the graph has not been modified.
    pub fn remove_edge(&mut self, edge_id: EdgeID) -> Result<(), ()> {
        if self.edges.remove(edge_id).is_none() {
            return Err(());
        }

        self.needs_compile = true;

        Ok(())
    }
}

impl<PortType: Enum> AudioGraphHelper<PortType> {
    /// Compile the graph into a schedule.
    pub fn compile(&mut self) -> Result<CompiledSchedule<PortType>, CompileGraphError> {
        self.needs_compile = false;

        super::compile::<PortType>(self.nodes.values(), self.edges.values())
    }

    /// Returns `true` if `AudioGraphHelper::compile()` should be called
    /// again because the state of the graph has changed since the last
    /// compile.
    pub fn needs_compile(&self) -> bool {
        self.needs_compile
    }

    fn remove_edges_with_port(&mut self, node_id: NodeID, port_id: PortID) -> Vec<EdgeID> {
        let mut edges_to_remove: Vec<EdgeID> = Vec::new();

        // Remove all existing edges which have this port.
        for edge in self.edges.values() {
            if (edge.src_node == node_id && edge.src_port == port_id)
                || (edge.dst_node == node_id && edge.dst_port == port_id)
            {
                edges_to_remove.push(edge.id);
            }
        }

        for edge_id in edges_to_remove.iter() {
            self.edges.remove(*edge_id);
        }

        edges_to_remove
    }

    fn cycle_detected(&self) -> bool {
        GraphIR::<PortType>::preprocess(self.nodes.values(), self.edges.values())
            .unwrap()
            .tarjan()
            > 0
    }
}
