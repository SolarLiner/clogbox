pub mod sample;
pub mod analysis;
mod utilitarian;

use crate::r#enum::{Enum, EnumMapMut, EnumMapRef};
use az::CastFrom;
use std::marker::PhantomData;
use std::ops;
use numeric_array::ArrayLength;
use numeric_array::generic_array::GenericArray;
use numeric_array::generic_array::sequence::GenericSequence;
use typenum::Unsigned;

#[derive(Debug, Copy, Clone)]
pub struct StreamData {
    pub sample_rate: f64,
    pub bpm: f64,
    pub block_size: usize,
}

impl StreamData {
    pub fn dt(&self) -> f64 {
        self.sample_rate.recip()
    }

    pub fn beat_length(&self, beats: f64) -> f64 {
        beats * self.bpm.recip() / 60.
    }

    pub fn beat_sample_length(&self, beats: f64) -> f64 {
        self.sample_rate * self.beat_length(beats)
    }
}

#[derive(Debug)]
pub struct ModuleCtxRaw<'a, T> {
    stream_data: StreamData,
    inputs: &'a [&'a [T]],
    outputs: &'a mut [&'a mut [T]],
}

impl<'a, T> ModuleCtxRaw<'a, T> {
    pub fn dt(&self) -> f64 {
        self.stream_data.dt()
    }
    pub fn fork<U>(
        &self,
        inputs: &'a [&'a [U]],
        outputs: &'a mut [&'a mut [U]],
    ) -> ModuleCtxRaw<'a, U> {
        ModuleCtxRaw {
            stream_data: self.stream_data,
            inputs,
            outputs,
        }
    }

    pub fn input_raw(&self, i: usize) -> &'a [T] {
        self.inputs[i]
    }

    pub fn output_raw(&mut self, i: usize) -> &mut [T] {
        self.outputs[i]
    }
    
    pub fn in_out_raw(&mut self, i: usize, j: usize) -> (&[T], &mut [T]) {
        (&self.inputs[i], &mut self.outputs[j])
    }
}

#[allow(unused_variables)]
pub trait RawModule: Send {
    type Sample;
    
    fn inputs(&self) -> usize;
    fn outputs(&self) -> usize;

    fn supports_stream(&self, data: StreamData) -> bool;

    fn reallocate(&mut self, stream_data: StreamData) {}

    fn reset(&mut self) {}

    fn process(&mut self, context: &mut ModuleCtxRaw<Self::Sample>) -> ProcessStatus;
}

pub type ModuleContext<'a, M> =
    ModCtxImpl<'a, <M as Module>::Sample, <M as Module>::Inputs, <M as Module>::Outputs>;

#[derive(Debug)]
#[repr(transparent)]
pub struct ModCtxImpl<'a, T, In, Out> {
    raw: ModuleCtxRaw<'a, T>,
    __io: PhantomData<(In, Out)>,
}

impl<'a, T, In, Out> ops::Deref for ModCtxImpl<'a, T, In, Out> {
    type Target = ModuleCtxRaw<'a, T>;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<'a, T, In, Out> ops::DerefMut for ModCtxImpl<'a, T, In, Out> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.raw
    }
}

impl<'a, T, In: Enum, Out: Enum> ModCtxImpl<'a, T, In, Out> {
    pub fn from_raw(raw: ModuleCtxRaw<'a, T>) -> Self {
        assert!(
            raw.inputs.len() >= In::Count::USIZE,
            "Not enough inputs in context for module"
        );
        assert!(
            raw.outputs.len() >= Out::Count::USIZE,
            "Not enough outputs in context"
        );
        Self {
            raw,
            __io: PhantomData,
        }
    }

    pub fn input(&self, ix: In) -> &'a [T] {
        self.raw.input_raw(ix.cast())
    }

    pub fn output(&mut self, ix: Out) -> &mut [T] {
        self.raw.output_raw(ix.cast())
    }
    
    pub fn in_out(&mut self, inp: In, out: Out) -> (&[T], &mut [T]) {
        self.raw.in_out_raw(inp.cast(), out.cast())
    }
    
    pub fn fork<U, I2, O2>(&self, inputs: EnumMapRef<'a, I2, &'a [U]>, outputs: EnumMapMut<'a, O2, &'a mut [U]>) -> ModCtxImpl<'a, U, I2, O2> {
        ModCtxImpl {
            raw: self.raw.fork(inputs.into_inner(), outputs.into_inner()),
            __io: PhantomData,
        }
    }
}

pub enum ProcessStatus {
    Running,
    Tail(u64),
    Done,
}

#[allow(unused_variables)]
pub trait Module: 'static + Send {
    type Sample;
    type Inputs: Enum;
    type Outputs: Enum;

    fn supports_stream(&self, data: StreamData) -> bool;

    fn reallocate(&mut self, stream_data: StreamData) {}

    fn reset(&mut self) {}

    #[inline]
    fn latency(&self) -> GenericArray<f64, <Self::Outputs as Enum>::Count> where <Self::Outputs as Enum>::Count: ArrayLength {
        GenericArray::generate(|_| 0.0)
    }

    fn process(&mut self, context: &mut ModuleContext<Self>) -> ProcessStatus;
}

impl<M: Module> RawModule for M {
    type Sample = ();

    #[inline]
    fn inputs(&self) -> usize {
        <M::Inputs as Enum>::Count::USIZE
    }

    #[inline]
    fn outputs(&self) -> usize {
        <M::Outputs as Enum>::Count::USIZE
    }

    #[inline]
    fn supports_stream(&self, data: StreamData) -> bool {
        M::supports_stream(self, data)
    }

    #[inline]
    fn reallocate(&mut self, stream_data: StreamData) {
        M::reallocate(self, stream_data)
    }

    #[inline]
    fn reset(&mut self) {
        M::reset(self)
    }

    #[inline]
    fn process(&mut self, raw_context: &mut ModuleCtxRaw<Self::Sample>) -> ProcessStatus {
        // Safety: ModuleCtxImpl<'a, Self::Sample, ...> is #[repr(transparent)]
        let context = unsafe { std::mem::transmute::<&mut ModuleCtxRaw<Self::Sample>, &mut ModuleContext<M>>(raw_context) };
        M::process(self, context)
    }
}

pub trait ModuleConstructor {
    type Module: Module;
    fn allocate(&self, stream_data: StreamData) -> Self::Module;
}

impl<'a, M: ModuleConstructor> ModuleConstructor for &'a M {
    type Module = M::Module;

    #[inline]
    fn allocate(&self, stream_data: StreamData) -> Self::Module {
        M::allocate(self, stream_data)
    }
}

pub struct ModuleCloner<M> {
    module: M,
}

impl<M: Module + Clone> ModuleConstructor for ModuleCloner<M> {
    type Module = M;

    #[inline]
    fn allocate(&self, _: StreamData) -> Self::Module {
        self.module.clone()
    }
}