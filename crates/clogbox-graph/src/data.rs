use crate::{EdgeId, NodeId};
use slotmap::SecondaryMap;
use std::ops;

/// Type of data associated with a graph. Use this alongside a graph to store arbitrary data in 
/// a map.
#[derive(Debug)]
pub struct GraphData<N, E> {
    /// Node data mapped by [`NodeId`].
    pub nodes: SecondaryMap<NodeId, N>,
    /// Edge data mapped by [`EdgeId`].
    pub edges: SecondaryMap<EdgeId, E>,
}

impl<N, E> Default for GraphData<N, E> {
    fn default() -> Self {
        Self {
            nodes: SecondaryMap::default(),
            edges: SecondaryMap::default(),
        }
    }
}

impl<N, E> ops::Index<NodeId> for GraphData<N, E> {
    type Output = N;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self.nodes[index]
    }
}

impl<N, E> ops::IndexMut<NodeId> for GraphData<N, E> {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        &mut self.nodes[index]
    }
}

impl<N, E> ops::Index<EdgeId> for GraphData<N, E> {
    type Output = E;

    fn index(&self, index: EdgeId) -> &Self::Output {
        &self.edges[index]
    }
}

impl<N, E> ops::IndexMut<EdgeId> for GraphData<N, E> {
    fn index_mut(&mut self, index: EdgeId) -> &mut Self::Output {
        &mut self.edges[index]
    }
}

impl<N, E> GraphData<N, E> {
    /// Create a new [`GraphData`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new data for this particular node. Returns the previously stored data, if there was
    /// one.
    /// 
    /// # Arguments 
    /// 
    /// * `id`: Node ID
    /// * `value`: Value to insert
    /// 
    pub fn add_node(&mut self, id: NodeId, value: N) -> Option<N> {
        self.nodes.insert(id, value)
    }
    
    /// Retrieve the data for the given node, if it exists.
    /// 
    /// # Arguments 
    /// 
    /// * `id`: Node ID
    /// 
    pub fn get_node(&self, id: NodeId) -> Option<&N> {
        self.nodes.get(id)
    }

    /// Add a new data for this edge. Returns the previously stored data, if there was one.
    /// 
    /// # Arguments 
    /// 
    /// * `id`: Edge ID
    /// * `value`: Value to insert.
    /// 
    pub fn add_edge(&mut self, id: EdgeId, value: E) -> Option<E> {
        self.edges.insert(id, value)
    }
    
    /// Retrieve the data for the given edge, if it exists.
    /// 
    /// # Arguments 
    /// 
    /// * `id`: Edge ID
    /// 
    /// returns: Option<&E> 
    /// 
    /// # Examples 
    /// 
    /// ```
    /// 
    /// ```
    pub fn get_edge(&self, id: EdgeId) -> Option<&E> {
        self.edges.get(id)
    }
}
