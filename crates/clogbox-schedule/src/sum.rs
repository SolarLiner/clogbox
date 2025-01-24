use crate::event::{merge_timestamps_dyn, EventBuffer};
use crate::storage::SharedStorage;
use num_traits::Zero;
use std::ops;

pub(crate) fn audio<T: Copy + Zero + ops::AddAssign + az::Cast<usize>>(
    inputs: impl SharedStorage<Value = [T]>,
    output: &mut [T],
) {
    for (i, out) in output.iter_mut().enumerate() {
        *out = (0..inputs.len())
            .map(|j| inputs.get(j)[i])
            .reduce(|mut a, b| {
                a += b;
                a
            })
            .unwrap_or_else(T::zero);
    }
}

pub(crate) fn events<T: Copy + PartialOrd>(
    inputs: impl SharedStorage<Value=EventBuffer<T>>,
    output: &mut EventBuffer<T>,
) -> Result<(), BufferOverflow> {
    output.clear();
    for event in merge_timestamps_dyn((0..inputs.len()).map(|i| *inputs.get(i))) {
        let Ok(()) = output.push(event.sample, *event.value) else {
            return Err(BufferOverflow);
        };
    }
    Ok(())
}

pub(crate) struct BufferOverflow;
