use crate::storage::SharedStorage;
use crate::{NoteBuffer, ParamBuffer};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::Enum;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Enum)]
pub enum SocketType {
    Audio,
    Param,
    Note,
}

pub type SocketCount = EnumMapArray<SocketType, usize>;

#[derive(Debug, Clone, Copy)]
pub struct Sockets {
    pub inputs: SocketCount,
    pub outputs: SocketCount,
}

#[derive(Debug, Clone, Copy)]
pub struct StreamData {
    pub beats_per_minute: f32,
    pub sample_rate: f32,
    pub buffer_size: usize,
}

#[derive(Copy, Clone)]
pub struct ExecutionContext<'a, T> {
    pub stream_data: &'a StreamData,
    pub audio_storage: &'a dyn SharedStorage<Value = [T]>,
    pub param_storage: &'a dyn SharedStorage<Value = ParamBuffer>,
    pub note_storage: &'a dyn SharedStorage<Value = NoteBuffer>,
}

pub enum ProcessStatus {
    Continue,
    Done,
}

pub trait RawModule {
    type Scalar;

    fn sockets(&self) -> Sockets;

    fn process(&self, ctx: &ExecutionContext<Self::Scalar>) -> ProcessStatus;
}
