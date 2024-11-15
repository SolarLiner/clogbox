use crate::graph::error::{AddEdgeError, AddPortError, CompileGraphError};
use crate::graph::{AudioGraphHelper, ModuleMap, NodeID, PortID};
use crate::schedule::Schedule;
use clogbox_core::module::{BufferStorage, Module, RawModule};
use clogbox_core::r#enum::az::Cast;
use clogbox_core::r#enum::{count, enum_iter, Enum, Sequential};
use clogbox_derive::Enum;
use num_traits::Zero;
use std::collections::HashMap;
use std::marker::PhantomData;
use typenum::U1;

mod graph;
mod schedule;

pub struct ScheduleBuilder<T> {
    audio_graph: AudioGraphHelper<PortType>,
    modules: ModuleMap<T>,
    io_nodes: Vec<IoNode>,
}

impl<T> Default for ScheduleBuilder<T> {
    fn default() -> Self {
        Self {
            audio_graph: AudioGraphHelper::new(),
            modules: ModuleMap::default(),
            io_nodes: vec![],
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct IoNode {
    id: NodeID,
    is_input: bool,
}

#[derive(Debug)]
pub struct Node<M> {
    id: NodeID,
    __module: PhantomData<M>,
}

impl<M> Copy for Node<M> {}

impl<M> Clone for Node<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Zero> ScheduleBuilder<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_io_node(&mut self, is_input: bool) -> Result<IoNode, AddPortError> {
        let id = self.audio_graph.add_node(0.0);
        self.audio_graph
            .add_port(id, PortID(0), PortType::Audio, !is_input)?;
        let node = IoNode { id, is_input };
        self.io_nodes.push(node);
        Ok(node)
    }

    pub fn add_node<M: Module<Sample = T>>(
        &mut self,
        module: impl Into<Box<M>>,
    ) -> Result<Node<M>, AddPortError> {
        let id = self.audio_graph.add_node(0.0);
        for inp in enum_iter::<M::Inputs>() {
            self.audio_graph
                .add_port(id, input_port::<M>(inp), PortType::Audio, true)?;
        }
        for out in enum_iter::<M::Outputs>() {
            self.audio_graph
                .add_port(id, output_port::<M>(out), PortType::Audio, false)?;
        }
        let module: Box<dyn RawModule<Sample = T>> = module.into();
        self.modules.insert(id, module);
        Ok(Node {
            id,
            __module: PhantomData,
        })
    }

    pub fn connect<M1: Module, M2: Module>(
        &mut self,
        source: Node<M1>,
        target: Node<M2>,
        input: M1::Inputs,
        output: M2::Outputs,
    ) -> Result<(), AddEdgeError> {
        self.audio_graph.add_edge(
            source.id,
            input_port::<M1>(input),
            target.id,
            output_port::<M2>(output),
            true,
        )?;
        Ok(())
    }

    pub fn connect_input<M: Module>(
        &mut self,
        source: IoNode,
        target: Node<M>,
        input: M::Inputs,
    ) -> Result<(), AddEdgeError> {
        self.audio_graph.add_edge(
            source.id,
            PortID(0),
            target.id,
            input_port::<M>(input),
            true,
        )?;
        Ok(())
    }

    pub fn connect_output<M: Module>(
        &mut self,
        source: Node<M>,
        target: IoNode,
        output: M::Outputs,
    ) -> Result<(), AddEdgeError> {
        self.audio_graph.add_edge(
            source.id,
            output_port::<M>(output),
            target.id,
            PortID(0),
            true,
        )?;
        Ok(())
    }
}

impl<T: Zero> ScheduleBuilder<T> {
    pub fn compile(mut self, max_buffer_size: usize) -> Result<Schedule<T>, CompileGraphError> {
        let schedule = self.audio_graph.compile()?;
        let max_buffers = schedule.num_buffers[PortType::Audio];
        let buffers = std::iter::repeat_with(|| {
            std::iter::repeat_with(T::zero)
                .take(max_buffer_size)
                .collect::<Box<[_]>>()
        })
        .take(max_buffers)
        .collect::<Box<[_]>>();
        Ok(Schedule {
            schedule,
            input_nodes: self
                .io_nodes
                .iter()
                .filter(|n| n.is_input)
                .map(|n| n.id)
                .collect(),
            output_nodes: self
                .io_nodes
                .iter()
                .filter(|n| !n.is_input)
                .map(|n| n.id)
                .collect(),
            modules: self.modules,
            buffers,
            max_buffer_size,
        })
    }
}

fn input_port<M: Module>(input: M::Inputs) -> PortID {
    PortID(input.cast() as u32)
}

fn output_port<M: Module>(output: M::Outputs) -> PortID {
    PortID((count::<M::Inputs>() + output.cast()) as u32)
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Enum)]
#[cfg_attr(feature = "serialize", derive(serde::Deserialize, serde::Serialize))]
pub enum PortType {
    Audio,
}
