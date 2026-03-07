use std::sync::Arc;

mod rb;
#[cfg(test)]
mod tests;

pub struct Producer<T> {
    rb: Arc<rb::RingBuffer<T>>,
}

unsafe impl<T: Send> Send for Producer<T> {}

impl<T> Producer<T> {
    pub fn len(&self) -> usize {
        self.rb.len()
    }
    pub fn capacity(&self) -> usize {
        self.rb.capacity()
    }
    pub fn free_slots(&self) -> usize {
        self.rb.free_slots()
    }

    pub fn is_empty(&self) -> bool {
        self.rb.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.rb.is_full()
    }

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
        if let Some(to_drop) = items.len().checked_sub(self.rb.free_slots()) {
            self.rb.drop_items(to_drop);
        }

        self.push_slice(items);
    }
}

pub struct Consumer<T> {
    rb: Arc<rb::RingBuffer<T>>,
}

unsafe impl<T: Send> Send for Consumer<T> {}

impl<T> Consumer<T> {
    pub fn len(&self) -> usize {
        self.rb.len()
    }
    pub fn capacity(&self) -> usize {
        self.rb.capacity()
    }
    pub fn free_slots(&self) -> usize {
        self.rb.free_slots()
    }

    pub fn is_empty(&self) -> bool {
        self.rb.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.rb.is_full()
    }

    pub fn read_pos(&self) -> usize {
        self.rb.read_pos()
    }

    pub fn write_pos(&self) -> usize {
        self.rb.write_pos()
    }

    pub fn pop(&self) -> Option<T> {
        self.rb.pop()
    }

    pub fn pop_slice(&self, slice: &mut [T]) -> usize
    where
        T: Copy,
    {
        self.rb.pop_slice(slice)
    }

    pub fn iter(&self) -> impl '_ + Iterator<Item = &T> {
        self.rb.reload_indices();
        (0..self.rb.len()).map(move |i| &self.rb[i])
    }

    pub fn drain(&self) -> impl '_ + Iterator<Item = T> {
        std::iter::from_fn(move || self.pop())
    }
}

pub fn create<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    let rb = Arc::new(rb::RingBuffer::new(capacity));
    (Producer { rb: rb.clone() }, Consumer { rb })
}
