use crate::eventbuffer::EventSlice;
use clogbox_enum::Enum;
use clogbox_math::recip::Recip;
use std::num::NonZeroU32;
use context::ProcessContext;

pub mod context;
pub mod contrib;
pub mod eventbuffer;
pub mod macros;
pub mod modules;
pub mod note;
pub mod r#dyn;
pub mod sample;

pub type Samplerate = Recip<f64>;
pub type ParamSlice = EventSlice<f32>;

pub type NoteSlice = EventSlice<note::NoteEvent>;

#[derive(Debug, Copy, Clone)]
pub struct PrepareResult {
    pub latency: f64,
}

#[derive(Debug, Copy, Clone)]
pub struct ProcessResult {
    pub tail: Option<NonZeroU32>,
}

pub trait Module {
    type Sample;
    type AudioIn: Enum;
    type AudioOut: Enum;
    type ParamsIn: Enum;
    type ParamsOut: Enum;
    type NoteIn: Enum;
    type NoteOut: Enum;

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult;

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult;
}
