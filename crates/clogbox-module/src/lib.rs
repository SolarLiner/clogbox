use clogbox_enum::Enum;
use clogbox_math::recip::Recip;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::ops;
use crate::eventbuffer::EventSlice;

pub mod r#dyn;
pub mod sample;
pub mod eventbuffer;
pub mod note;

pub type Samplerate = Recip<f64>;
pub type ParamSlice = EventSlice<f32>;

pub type NoteSlice = EventSlice<note::NoteEvent>;

pub struct StreamContext {
    pub sample_rate: Samplerate,
    pub block_size: usize,
}

pub struct ProcessContext<'a, M: ?Sized + Module> {
    pub audio_in: &'a dyn ops::Index<M::AudioIn, Output = [M::Sample]>,
    pub audio_out: &'a mut dyn ops::IndexMut<M::AudioOut, Output = [M::Sample]>,
    pub params_in: &'a dyn ops::Index<M::ParamsIn, Output = ParamSlice>,
    pub params_out: &'a mut dyn ops::IndexMut<M::ParamsOut, Output = ParamSlice>,
    pub note_in: &'a dyn ops::Index<M::NoteIn, Output = NoteSlice>,
    pub note_out: &'a mut dyn ops::IndexMut<M::NoteOut, Output = NoteSlice>,
    pub stream_context: &'a StreamContext,
    pub __phantom: PhantomData<&'a M>,
}

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

