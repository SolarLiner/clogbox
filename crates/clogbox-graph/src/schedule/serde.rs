use crate::graph::{ModuleMap, NodeID};
use crate::{graph, PortType};
use clogbox_core::module::RawModule;
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use slotmap::SecondaryMap;
use std::collections::HashSet;

/// Serializes a schedule to a writer using CBOR format.
///
/// # Arguments
///
/// * `writer` - The writer to which the serialized data will be written.
/// * `schedule` - The schedule to serialize.
/// * `serialize_module` - A function that serializes a module to a `serde_json::Value`.
///
/// # Returns
///
/// * `Result<(), serde_cbor::Error>` - Returns `Ok(())` if successful, or an error if serialization fails.
pub fn serialize<T>(
    writer: impl std::io::Write,
    schedule: &super::Schedule<T>,
    serialize_module: impl Fn(&dyn RawModule<Sample = T>) -> serde_json::Value,
) -> Result<(), serde_cbor::Error> {
    let serialized = SerializedSchedule::serialize(schedule, serialize_module);
    serde_cbor::to_writer(writer, &serialized)
}

/// Deserializes a schedule from a reader using CBOR format.
///
/// # Arguments
///
/// * `reader` - The reader from which the serialized data will be read.
/// * `max_buffer_size` - The maximum buffer size for the schedule.
/// * `realize_module` - A function that deserializes a `serde_json::Value` to a module.
///
/// # Returns
///
/// * `Result<super::Schedule<T>, serde_cbor::Error>` - Returns the deserialized schedule if successful, or an error if deserialization fails.

pub fn deserialize<T: Zero>(
    reader: impl std::io::Read,
    max_buffer_size: usize,
    realize_module: impl Fn(serde_json::Value) -> Box<dyn RawModule<Sample = T>>,
) -> Result<super::Schedule<T>, serde_cbor::Error> {
    let serialized: SerializedSchedule = serde_cbor::from_reader(reader)?;
    Ok(serialized.deserialize(max_buffer_size, realize_module))
}

#[derive(Serialize, Deserialize)]
struct SerializedSchedule {
    schedule: graph::CompiledSchedule<PortType>,
    modules_data: SecondaryMap<NodeID, serde_json::Value>,
    input_nodes: HashSet<NodeID>,
    output_nodes: HashSet<NodeID>,
}

impl SerializedSchedule {
    fn serialize<T>(
        schedule: &super::Schedule<T>,
        serialize_module: impl Fn(&dyn RawModule<Sample = T>) -> serde_json::Value,
    ) -> Self {
        let modules_data = schedule
            .modules
            .iter()
            .map(|(id, module)| (id, serialize_module(&**module)))
            .collect::<SecondaryMap<_, _>>();
        SerializedSchedule {
            schedule: schedule.schedule.clone(),
            modules_data,
            input_nodes: schedule.input_nodes.clone(),
            output_nodes: schedule.output_nodes.clone(),
        }
    }

    fn deserialize<T: Zero>(
        self,
        max_buffer_size: usize,
        realize_module: impl Fn(serde_json::Value) -> Box<dyn RawModule<Sample = T>>,
    ) -> super::Schedule<T> {
        // Realize the schedule, replacing the serialized module data with the actual module instances.
        let modules = self
            .modules_data
            .into_iter()
            .map(|(id, data)| (id, realize_module(data)))
            .collect::<ModuleMap<T>>();
        let num_buffers = self.schedule.num_buffers[PortType::Audio];
        let buffers = std::iter::repeat_with(|| {
            std::iter::repeat_with(T::zero)
                .take(max_buffer_size)
                .collect::<Box<[_]>>()
        })
        .take(num_buffers)
        .collect::<Box<[_]>>();

        super::Schedule {
            schedule: self.schedule,
            input_nodes: self.input_nodes,
            output_nodes: self.output_nodes,
            modules,
            buffers,
            max_buffer_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clogbox_core::module::{BufferStorage, Module, ModuleContext, ProcessStatus, StreamData};
    use clogbox_core::param::{Params, EMPTY_PARAMS};
    use clogbox_core::r#enum::enum_map::EnumMapArray;
    use clogbox_core::r#enum::{seq, Empty, Sequential};
    use serde_json::json;
    use std::io::Cursor;
    use std::sync::Arc;
    use typenum::U1;

    struct MockRawModule;

    impl Module for MockRawModule {
        type Sample = f32;
        type Inputs = Sequential<U1>;
        type Outputs = Sequential<U1>;
        type Params = Empty;

        fn get_params(&self) -> Arc<impl '_ + Params<Params= Self::Params>> {
            Arc::new(EMPTY_PARAMS)
        }

        fn supports_stream(&self, data: StreamData) -> bool {
            true
        }

        fn latency(
            &self,
            input_latencies: EnumMapArray<Self::Inputs, f64>,
        ) -> EnumMapArray<Self::Outputs, f64> {
            input_latencies
        }

        fn process<
            S: BufferStorage<Sample = Self::Sample, Input = Self::Inputs, Output = Self::Outputs>,
        >(
            &mut self,
            context: &mut ModuleContext<S>,
        ) -> ProcessStatus {
            ProcessStatus::Running
        }
    }

    fn mock_serialize_module(module: &dyn RawModule<Sample = f32>) -> serde_json::Value {
        json!({"mock": "data"})
    }

    fn mock_realize_module(_data: serde_json::Value) -> Box<dyn RawModule<Sample = f32>> {
        Box::new(MockRawModule)
    }

    fn generate_schedule() -> crate::Schedule<f32> {
        let mut graph = crate::ScheduleBuilder::new();
        let node = graph.add_node(MockRawModule).unwrap();
        let global_in = graph.add_io_node(true).unwrap();
        let global_out = graph.add_io_node(false).unwrap();
        graph.connect_input(global_in, node, seq(0)).unwrap();
        graph.connect_output(node, global_out, seq(0)).unwrap();
        graph.compile(64).unwrap()
    }

    #[test]
    fn serialize_deserialize_round_trip() {
        let schedule = generate_schedule();
        let mut buffer = Vec::new();

        serialize(&mut buffer, &schedule, mock_serialize_module).unwrap();
        let deserialized_schedule =
            deserialize(Cursor::new(buffer), 1024, mock_realize_module).unwrap();

        assert!(
            schedule == deserialized_schedule,
            "Schedule round-trip serialization failed"
        );
    }

    #[test]
    fn serialize_deserialize_with_modules() {
        let schedule = generate_schedule();
        let mut buffer = Vec::new();

        serialize(&mut buffer, &schedule, mock_serialize_module).unwrap();
        let deserialized_schedule =
            deserialize(Cursor::new(buffer), 1024, mock_realize_module).unwrap();

        assert_eq!(schedule.modules.len(), deserialized_schedule.modules.len());
    }
}
