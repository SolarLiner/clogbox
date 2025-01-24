use crate::module::{RawModule, SocketType};
use clogbox_enum::enum_iter;
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_graph::{EdgeId, GraphBase, NodeId, OwnedGraph};
use slotmap::SecondaryMap;
use std::collections::HashSet;
use crate::ScheduleSerialized;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Node {
    inputs: EnumMapArray<SocketType, Vec<NodeId>>,
    outputs: EnumMapArray<SocketType, Vec<NodeId>>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Input(NodeId);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Output(NodeId);

impl Node {
    pub fn input(&self, typ: SocketType, index: usize) -> InputConnection {
        Connection {
            id: self.inputs[typ][index],
            conn_type: typ,
        }
    }

    pub fn output(&self, typ: SocketType, index: usize) -> OutputConnection {
        Connection {
            id: self.outputs[typ][index],
            conn_type: typ,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Connection<const IS_INPUT: bool> {
    id: NodeId,
    conn_type: SocketType,
}

pub type InputConnection = Connection<true>;
pub type OutputConnection = Connection<false>;

pub struct ScheduleBuilder<T> {
    graph: GraphBase,
    modules: SecondaryMap<NodeId, Box<dyn RawModule<Scalar = T>>>,
    inputs: EnumMapArray<SocketType, HashSet<NodeId>>,
    outputs: EnumMapArray<SocketType, HashSet<NodeId>>,
}

impl<T> Default for ScheduleBuilder<T> {
    fn default() -> Self {
        Self {
            graph: GraphBase::new(),
            modules: SecondaryMap::new(),
            inputs: EnumMapArray::new(|typ| HashSet::new()),
            outputs: EnumMapArray::new(|typ| HashSet::new()),
        }
    }
}

impl<T> ScheduleBuilder<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_module(&mut self, module: impl 'static + RawModule<Scalar = T>) -> Node {
        let sockets = module.sockets();
        let mut node = Node {
            inputs: EnumMapArray::new(|typ| Vec::new()),
            outputs: EnumMapArray::new(|typ| Vec::new()),
        };
        for typ in enum_iter::<SocketType>() {
            for _ in (0..sockets.inputs[typ]) {
                node.inputs[typ].push(self.graph.add_node());
            }
            for _ in (0..sockets.inputs[typ]) {
                node.outputs[typ].push(self.graph.add_node());
            }
        }
        node
    }

    pub fn add_input(&mut self, typ: SocketType) -> Input {
        let id = self.graph.add_node();
        self.inputs[typ].insert(id);
        Input(id)
    }

    pub fn add_output(&mut self, typ: SocketType) -> Output {
        let id = self.graph.add_node();
        self.outputs[typ].insert(id);
        Output(id)
    }

    pub fn connect(
        mut self,
        output: OutputConnection,
        input: InputConnection,
    ) -> Result<EdgeId, ConnectionTypeMismatch> {
        if input.conn_type != output.conn_type {
            return Err(ConnectionTypeMismatch {
                input_type: input.conn_type,
                output_type: output.conn_type,
            });
        }

        Ok(self.graph.add_edge(output.id, input.id))
    }

    pub fn connect_input(
        &mut self,
        Input(input): Input,
        module_input: InputConnection,
    ) -> Result<EdgeId, ConnectionTypeMismatch> {
        if !self.inputs[module_input.conn_type].contains(&input) {
            let input_type = [SocketType::Audio, SocketType::Param, SocketType::Note]
                .into_iter()
                .find(|&typ| self.inputs[typ].contains(&input))
                .unwrap();
            return Err(ConnectionTypeMismatch {
                input_type,
                output_type: module_input.conn_type,
            });
        }

        Ok(self.graph.add_edge(input, module_input.id))
    }
    
    pub fn connect_output(&mut self, Output(output): Output, module_output: OutputConnection) -> Result<EdgeId, ConnectionTypeMismatch> {
        if !self.outputs[module_output.conn_type].contains(&output) {
            let output_type = [SocketType::Audio, SocketType::Param, SocketType::Note]
                .into_iter()
                .find(|&typ| self.outputs[typ].contains(&output))
                .unwrap();
            return Err(ConnectionTypeMismatch {
                input_type: module_output.conn_type,
                output_type,
            });
        }
        
        Ok(self.graph.add_edge(output, module_output.id))
    }
    
    pub fn compile(&self) -> Result<ScheduleSerialized<T>, ()> {
        todo!()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ConnectionTypeMismatch {
    pub input_type: SocketType,
    pub output_type: SocketType,
}
