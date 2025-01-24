use crate::event::EventBuffer;
use crate::sum::BufferOverflow;
use crate::Timestamped;
use clogbox_math::interpolation::{InterpolateSingle, Linear};
use num_traits::{Float, Zero};
use numeric_array::NumericArray;
use std::collections::VecDeque;

pub struct AudioDelay<T> {
    delay_fract: T,
    buffer: VecDeque<T>,
}

impl<T: Float + az::Cast<usize>> AudioDelay<T> {
    pub(crate) fn process_sample(&mut self, sample: T) -> T {
        let m = self.buffer[self.buffer.len() - 2];
        let n = self.buffer[self.buffer.len() - 1];
        let vals = NumericArray::from([m, n]);
        let ret = Linear.interpolate_single(&vals, self.delay_fract);

        self.buffer.push_back(sample);
        ret
    }
    
    pub(crate) fn process_buffer(&mut self, input: &[T], output: &mut [T]) {
        for (i, o) in input.iter().copied().zip(output.iter_mut()) {
            *o = self.process_sample(i);
        }
    }
}

impl<T: Float + az::Cast<usize>> AudioDelay<T> {
    pub fn new(sample_amt: T) -> Self {
        let size = sample_amt.ceil().cast();
        let delay_fract = sample_amt.fract();
        Self {
            delay_fract,
            buffer: VecDeque::with_capacity(size),
        }
    }
}

pub struct EventDelay<T> {
    amount: usize,
    staging: EventBuffer<T>,
}

impl<T> EventDelay<T> {
    pub(crate) fn new(capacity: usize, delay_amount: usize) -> Self {
        Self {
            amount: delay_amount,
            staging: EventBuffer::new(capacity),
        }
    }
}

impl<T: Copy + PartialOrd> EventDelay<T> {
    pub(crate) fn process_buffer(
        &mut self,
        buffer_length: usize,
        input: &EventBuffer<T>,
        output: &mut EventBuffer<T>,
    ) -> Result<(), BufferOverflow> {
        for event in self
            .staging
            .iter_events()
            .take_while(|t| t.sample < buffer_length)
        {
            let Ok(()) = output.push(event.sample, *event.value) else {
                return Err(BufferOverflow);
            };
        }

        let it = input.iter_events().map(|t| Timestamped {
            sample: t.sample + self.amount,
            value: *t.value,
        });
        for event in it {
            if event.sample < buffer_length {
                let Ok(()) = output.push(event.sample, event.value) else {
                    return Err(BufferOverflow);
                };
            } else {
                let Ok(()) = self.staging.push(event.sample, event.value) else {
                    return Err(BufferOverflow);
                };
            }
        }

        Ok(())
    }
}
