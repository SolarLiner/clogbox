use audio_graph::error::{AddEdgeError, AddPortError, CompileGraphError};
use audio_graph::{AudioGraphHelper, NodeID, PortID, ScheduleEntry, TypeIdx};
use clogbox_core::module::{BufferStorage, MappedBufferStorage, Module, ModuleContext, OwnedBufferStorage, ProcessStatus, RawModule, RawModuleStorage, StreamData};
use clogbox_core::r#enum::az::Cast;
use clogbox_core::r#enum::{enum_iter, Either};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

struct StoredModule<T> {
    mapper: Box<dyn FnOnce() -> Box<dyn _>>,
    module: dyn RawModule<Sample=T>
}

pub struct Schedule {
    schedule: audio_graph::CompiledSchedule,
    input_nodes: HashSet<NodeID>,
    output_nodes: HashSet<NodeID>,
    modules: HashMap<NodeID, Box<StoredModule<f32>>>,
    buffers: OwnedBufferStorage<f32>,
    max_buffer_size: usize,
}

impl RawModule for Schedule {
    type Sample = f32;

    fn inputs(&self) -> usize {
        self.input_nodes.len()
    }
    
    fn outputs(&self) -> usize {
        self.output_nodes.len()
    }

    fn supports_stream(&self, data: StreamData) -> bool {
        self.max_buffer_size >= data.block_size && self.modules.values().all(|m| m.supports_stream(data))
    }

    fn reallocate(&mut self, stream_data: StreamData) {
        let max_buffers = self.schedule.num_buffers.iter().copied().max().unwrap_or(0);
        self.buffers = OwnedBufferStorage::new(max_buffers, max_buffers, stream_data.block_size);
    }

    fn reset(&mut self) {
    }

    fn process(&mut self, context: &mut ModuleContext<RawModuleStorage<Self::Sample>>) -> ProcessStatus {
        for entry in &self.schedule.schedule {
            match entry {
                ScheduleEntry::Node(node) => {
                    let mut storage = MappedBufferStorage {
                        storage: &mut self.buffers,
                        mapper: |x: Either<_, _>| match x {
                            Either::Left(input) => node.input_buffers[input.cast()].buffer_index.0,
                            Either::Right(output) => node.output_buffers[output.cast()]
                                .buffer_index.0,
                        },
                        __io_types: PhantomData,
                    };
                    for assignment in &node.input_buffers {
                        if assignment.should_clear {
                            self.buffers.clear_input(assignment.buffer_index.0);
                        }
                    }
                    for assignment in &node.output_buffers {
                        if assignment.should_clear {
                            self.buffers.clear_output(assignment.buffer_index.0);
                        }
                    }
                    self.modules[&node.id].process(&mut ModuleContext {
                        stream_data: context.stream_data,
                        buffers: &mut storage,
                    });
                }
                ScheduleEntry::Delay(_) => {}
                ScheduleEntry::Sum(_) => {}
            }
        }
        ProcessStatus::Running
    }
}

pub struct ScheduleBuilder {
    audio_graph: AudioGraphHelper,
    modules: HashMap<NodeID, Box<dyn RawModule<Sample = f32>>>,
    io_nodes: Vec<IoNode>,
}

impl Default for ScheduleBuilder {
    fn default() -> Self {
        Self {
            audio_graph: AudioGraphHelper::new(2),
            modules: HashMap::new(),
            io_nodes: vec![],
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct IoNode {
    id: NodeID,
    is_input: bool,
}

#[derive(Debug, Copy, Clone)]
pub struct Node<M> {
    id: NodeID,
    __module: PhantomData<M>,
}

impl ScheduleBuilder {
    const TYPE_AUDIO: TypeIdx = TypeIdx(0);
    const TYPE_PARAM: TypeIdx = TypeIdx(1);

    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_io_node(&mut self, is_input: bool) -> Result<IoNode, AddPortError> {
        let id = self.audio_graph.add_node(0.0);
        self.audio_graph
            .add_port(id, PortID(0), Self::TYPE_AUDIO, is_input)?;
        let node = IoNode { id, is_input };
        self.io_nodes.push(node);
        Ok(node)
    }

    pub fn add_node<M: Module<Sample=f32>>(
        &mut self,
        module: impl Into<Box<M>>,
    ) -> Result<Node<M>, AddPortError> {
        let id = self.audio_graph.add_node(0.0);
        for inp in enum_iter::<M::Inputs>() {
            let port = PortID(inp.cast() as u32);
            self.audio_graph
                .add_port(id, port, Self::TYPE_AUDIO, true)?;
        }
        for out in enum_iter::<M::Outputs>() {
            let port = PortID(out.cast() as u32);
            self.audio_graph
                .add_port(id, port, Self::TYPE_AUDIO, false)?;
        }
        let module: Box<dyn RawModule<Sample = f32>> = module.into();
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
        let source_port = PortID(input.cast() as u32);
        let target_port = PortID(output.cast() as u32);
        self.audio_graph
            .add_edge(source.id, source_port, target.id, target_port, true)?;
        Ok(())
    }

    pub fn compile(mut self, max_buffer_size: usize) -> Result<Schedule, CompileGraphError> {
        let schedule = self.audio_graph.compile()?;
        todo!()
    }
}
