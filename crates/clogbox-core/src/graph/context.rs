use crate::graph::module::StreamData;
use crate::graph::module::{Module, ModuleError};
use crate::graph::slots::Slots;
use crate::graph::storage::{GraphStorage, SlotRef, SlotRefMut, StorageBorrow};
use crate::graph::{ControlBuffer, NoteBuffer, SlotType};
use derive_more::Debug;

pub type GraphContext<'a, M> =
    GraphContextImpl<'a, <M as Module>::Sample, <M as Module>::Inputs, <M as Module>::Outputs>;

/// Context of the graph exposed to individual modules.
#[derive(Debug)]
pub struct GraphContextImpl<'a, T, In, Out> {
    pub(crate) stream_data: &'a StreamData,
    #[debug(skip)]
    pub(crate) input_index: &'a dyn Fn(In) -> Option<(SlotType, usize)>,
    #[debug(skip)]
    pub(crate) output_index: &'a dyn Fn(Out) -> Option<(SlotType, usize)>,
    pub(crate) storage: &'a GraphStorage<T>,
}

impl<'a, T, In, Out> GraphContextImpl<'a, T, In, Out> {
    pub fn stream_data(&self) -> &'a StreamData {
        self.stream_data
    }
}

impl<T, In: Slots, Out: Slots> GraphContextImpl<'_, T, In, Out> {
    pub fn get_input(&self, input: In) -> Option<SlotRef<T>> {
        let (typ, index) = (self.input_index)(input)?;
        assert_eq!(typ, input.slot_type());
        Some(self.storage.get_buffer(typ, index))
    }

    pub fn get_audio_input(&self, input: In) -> Result<StorageBorrow<&[T]>, ModuleError> {
        let Some(buf) = self
            .get_input(input)
            .and_then(|r| r.map(|s| s.to_audio_buffer()).transpose())
        else {
            use std::io::Write;
            let mut storage = [0u8; 512];
            write!(&mut storage as &mut [u8], "{}", input.name())
                .map_err(|err| ModuleError::Fatal(err.into()))?;
            return Err(ModuleError::MissingRequiredInput(
                SlotType::Audio,
                input.name().to_string(),
            ));
        };
        Ok(buf)
    }

    pub fn get_control_input(&self, input: In) -> Result<StorageBorrow<&ControlBuffer>, ModuleError> {
        let Some(events) = self
            .get_input(input)
            .and_then(|r| r.map(|s| s.to_control_events()).transpose())
        else {
            return Err(ModuleError::MissingRequiredInput(
                SlotType::Control,
                input.name().to_string(),
            ));
        };
        Ok(events)
    }

    pub fn get_note_input(&self, input: In) -> Result<StorageBorrow<&NoteBuffer>, ModuleError> {
        let Some(events) = self
            .get_input(input)
            .and_then(|r| r.map(|s| s.to_note_events()).transpose())
        else {
            return Err(ModuleError::MissingRequiredInput(
                SlotType::Note,
                input.name().to_string(),
            ));
        };
        Ok(events)
    }

    pub fn get_output(&self, output: Out) -> Option<SlotRefMut<T>> {
        let (typ, index) = (self.output_index)(output)?;
        assert_eq!(typ, output.slot_type());
        Some(self.storage.get_buffer_mut(typ, index))
    }

    pub fn get_audio_output(&self, output: Out) -> Result<StorageBorrow<&mut [T]>, ModuleError> {
        let Some(buf) = self
            .get_output(output)
            .and_then(|m| m.map(|s| s.to_audio_buffer()).transpose())
        else {
            use std::io::Write;
            let mut storage = [0u8; 512];
            write!(&mut storage as &mut [u8], "{}", output.name())
                .map_err(|err| ModuleError::Fatal(err.into()))?;
            return Err(ModuleError::MissingRequiredOutput(
                SlotType::Audio,
                output.name().to_string(),
            ));
        };
        Ok(buf)
    }

    pub fn get_control_output(
        &self,
        output: Out,
    ) -> Result<StorageBorrow<&mut ControlBuffer>, ModuleError> {
        let Some(events) = self
            .get_output(output)
            .and_then(|m| m.map(|s| s.to_control_events()).transpose())
        else {
            return Err(ModuleError::MissingRequiredOutput(
                SlotType::Control,
                output.name().to_string(),
            ));
        };
        Ok(events)
    }

    pub fn get_note_output(&self, output: Out) -> Result<StorageBorrow<&mut NoteBuffer>, ModuleError> {
        let Some(events) = self
            .get_output(output)
            .and_then(|m| m.map(|s| s.to_note_events()).transpose())
        else {
            return Err(ModuleError::MissingRequiredOutput(
                SlotType::Note,
                output.name().to_string(),
            ));
        };
        Ok(events)
    }
}

pub struct RawGraphContext<'a, T> {
    pub(crate) storage: &'a GraphStorage<T>,
    pub(crate) stream_data: &'a StreamData,
    pub(crate) input_index: &'a dyn Fn(SlotType, usize) -> Option<(SlotType, usize)>,
    pub(crate) output_index: &'a dyn Fn(SlotType, usize) -> Option<(SlotType, usize)>,
}
