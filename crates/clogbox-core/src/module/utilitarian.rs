use crate::module::{Module, ModuleContext, ProcessStatus, StreamData};
use crate::r#enum::{enum_iter, CartesianProduct, Enum, EnumMap, EnumMapBox};
use num_traits::{Num, NumAssign, Zero};
use std::ops;
use typenum::Unsigned;

#[derive(Debug, Clone)]
pub struct ModMatrix<T, In, Out> {
    params: EnumMapBox<CartesianProduct<In, Out>, Box<[T]>>,
}

impl<T, In: Enum, Out: Enum> ModMatrix<T, In, Out>
where
    T: Copy,
    In::Count: ops::Mul<Out::Count>,
    <In::Count as ops::Mul<Out::Count>>::Output: Unsigned,
{
    pub fn new(
        block_size: usize,
        initial_values: EnumMapBox<CartesianProduct<In, Out>, T>,
    ) -> Self {
        Self {
            params: EnumMap::new(|k| {
                std::iter::repeat(initial_values[k])
                    .take(block_size)
                    .collect()
            }),
        }
    }

    pub fn set_param_block(&mut self, inp: In, out: Out, block: &[T]) {
        let param = CartesianProduct(inp, out);
        let arr = &mut self.params[param];
        
        let block = if block.len() > arr.len() {
            &block[..arr.len()]
        } else {
            block
        };
        
        arr[..block.len()].copy_from_slice(block);
        if block.len() < arr.len() {
            arr[block.len()..].fill(block[block.len() - 1]);
        }
    }
}

impl<
        T: 'static + Copy + Send + NumAssign + Num + Zero,
        In: 'static + Enum,
        Out: 'static + Enum,
    > Module for ModMatrix<T, In, Out>
where
    In::Count: ops::Mul<Out::Count, Output: Unsigned>,
{
    type Sample = T;
    type Inputs = In;
    type Outputs = Out;

    fn supports_stream(&self, data: StreamData) -> bool {
        self.params.values().all(|arr| arr.len() >= data.block_size)
    }

    fn reallocate(&mut self, stream_data: StreamData) {
        self.params = EnumMap::new(|_| vec![T::zero(); stream_data.block_size].into_boxed_slice());
    }

    #[inline]
    #[profiling::function]
    fn process(&mut self, context: &mut ModuleContext<Self>) -> ProcessStatus {
        let block_size = context.stream_data.block_size;
        for x in &mut *context.outputs {
            x.fill(T::zero());
        }

        for out in enum_iter::<Out>() {
            for inp in enum_iter::<In>() {
                let (in_buf, out_buf) = context.in_out(inp, out);
                let p = CartesianProduct(inp, out);
                let parr = &*self.params[p];
                // TODO: simd
                for i in 0..block_size {
                    let k = parr[i];
                    out_buf[i] += k * in_buf[i];
                }
            }
        }

        ProcessStatus::Running
    }
}
