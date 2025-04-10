use clogbox_enum::Enum;
use clogbox_math::recip::Recip;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::ops;

pub mod r#dyn;

pub type Samplerate = Recip<f64>;

pub struct ProcessContext<'a, M: ?Sized + Module> {
    pub audio_in: &'a dyn ops::Index<M::AudioIn, Output = [M::Sample]>,
    pub audio_out: &'a mut dyn ops::IndexMut<M::AudioOut, Output = [M::Sample]>,
    pub params_in: &'a dyn ops::Index<M::ParamsIn, Output = [M::Sample]>,
    pub params_out: &'a mut dyn ops::IndexMut<M::ParamsOut, Output = [M::Sample]>,
    pub note_in: &'a dyn ops::Index<M::NoteIn, Output = [M::Sample]>,
    pub note_out: &'a mut dyn ops::IndexMut<M::NoteOut, Output = [M::Sample]>,
    pub sample_rate: Samplerate,
    pub block_size: usize,
    __phantom: PhantomData<&'a M>,
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

