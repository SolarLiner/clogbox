use std::marker::PhantomData;
use std::sync::Arc;

pub mod rb;
#[cfg(test)]
mod tests;

pub use rb::RingBuffer;

pub struct Producer<T> {
    rb: Arc<RingBuffer<T>>,
}

unsafe impl<T> Send for Producer<T> {}

impl<T> Producer<T> {
    pub fn push(&self, item: T) -> Result<(), T> {
        self.rb.push(item)
    }

    pub fn push_slice(&self, items: &[T]) -> usize
    where
        T: Copy,
    {
        self.rb.push_slice(items)
    }

    pub fn push_overriding(&self, item: T) -> Option<T> {
        if let Err(item) = self.rb.push(item) {
            let old = self.rb.pop().unwrap();
            self.rb.push(item).ok().unwrap();
            Some(old)
        } else {
            None
        }
    }

    pub fn push_slice_overriding(&self, items: &[T])
    where
        T: Copy,
    {
        if let Some(to_drop) = items.len().checked_sub(self.rb.available()) {
            self.rb.drop_items(to_drop);
        }

        self.push_slice(items);
    }
}

pub struct Consumer<T> {
    rb: Arc<RingBuffer<T>>,
}

unsafe impl<T> Send for Consumer<T> {}

impl<T> Consumer<T> {
    pub fn pop(&self) -> Option<T> {
        self.rb.pop()
    }

    pub fn pop_slice(&self, slice: &mut [T]) -> usize
    where
        T: Copy,
    {
        self.rb.pop_slice(slice)
    }

    pub fn drain(&self) -> impl Iterator<Item = T> {
        std::iter::from_fn(move || self.pop())
    }
}

pub fn create<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    let rb = Arc::new(RingBuffer::new(capacity));
    (Producer { rb: rb.clone() }, Consumer { rb })
}
