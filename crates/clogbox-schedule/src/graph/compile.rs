use clogbox_graph::{algorithms, AdjacencyList, EdgeId, FromGraph, Graph, NodeId};
use num_traits::NumOps;
use slotmap::SecondaryMap;
use std::collections::HashSet;
use std::ops;

#[derive(Debug, Clone)]
struct IndexMap<T> {
    data: Vec<T>,
}

impl<T> ops::Index<usize> for IndexMap<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("Index out of bounds")
    }
}

impl<T> ops::IndexMut<usize> for IndexMap<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).expect("Index out of bounds")
    }
}

impl<T> Default for IndexMap<T> {
    fn default() -> Self {
        Self { data: Vec::new() }
    }
}

impl<T> IndexMap<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.data.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.data.get_mut(index)
    }
}

impl<T: Default> IndexMap<T> {
    pub fn insert(&mut self, index: usize, value: T) -> Option<T> {
        if index < self.data.len() {
            Some(std::mem::replace(&mut self.data[index], value))
        } else {
            while self.data.len() < index {
                self.data.push(T::default());
            }
            self.data.push(value);
            None
        }
    }
}

struct GraphIR<'a, T> {
    builder: &'a super::ScheduleBuilder<T>,
}

impl<'a, T> GraphIR<'a, T> {
    pub fn new(builder: &'a super::ScheduleBuilder<T>) -> Self {
        Self { builder }
    }

    fn schedule_for_output(&self, output: NodeId) -> SecondaryMap<EdgeId, usize> {
        let colors = algorithms::color_edges(&self.builder.graph);
        let groups = {
            let mut g = IndexMap::new();
            for (edge, &color) in &colors {
                if let Some(data) = g.get_mut(color) {
                    data.insert(edge);
                } else {
                    g.insert(1, HashSet::<_>::from_iter([edge]));
                }
            }
            g
        };
    }
}
