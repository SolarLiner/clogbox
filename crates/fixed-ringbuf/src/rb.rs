use crossbeam_utils::CachePadded;
use std::borrow::Cow;
use std::cell::{Cell, UnsafeCell};
use std::mem::MaybeUninit;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{fmt, ops};
use thread_local::ThreadLocal;

pub(crate) struct RingBuffer<T> {
    // Read and write indices are padded to avoid false sharing
    read_index: CachePadded<AtomicUsize>,
    // Thread-local cached read index for producers
    read_index_cached: ThreadLocal<Cell<usize>>,

    // Write index and its cached version for consumers
    write_index: CachePadded<AtomicUsize>,
    // Thread-local cached write index for consumers
    write_index_cached: ThreadLocal<Cell<usize>>,

    // Mask for efficient modulo operations (capacity must be power of 2)
    mask: usize,

    // The actual buffer storage
    buffer: Box<[UnsafeCell<MaybeUninit<T>>]>,
}

unsafe impl<T: Send> Send for RingBuffer<T> {}
unsafe impl<T: Sync> Sync for RingBuffer<T> {}

impl<T> UnwindSafe for RingBuffer<T> {}
impl<T> RefUnwindSafe for RingBuffer<T> {}

impl<T> fmt::Debug for RingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RingBuffer")
            .field(
                "read_index",
                &format!(
                    "<atomic: {}, cached: {}>",
                    self.read_index.load(Ordering::Relaxed),
                    self.read_index_cached
                        .get()
                        .map(|c| std::borrow::Cow::Owned(c.get().to_string()))
                        .unwrap_or(Cow::Borrowed("<uncached>"))
                ),
            )
            .field(
                "write_index",
                &format!(
                    "<atomic: {}, cached: {}>",
                    self.write_index.load(Ordering::Relaxed),
                    self.write_index_cached
                        .get()
                        .map(|c| std::borrow::Cow::Owned(c.get().to_string()))
                        .unwrap_or(Cow::Borrowed("<uncached>"))
                ),
            )
            .field("len", &self.len())
            .field("is_full", &self.is_full())
            .field("is_empty", &self.is_empty())
            .field("available", &self.free_slots())
            .field("read_pos", &self.read_pos())
            .field("write_pos", &self.write_pos())
            .field("mask", &format!("0x{:X}", self.mask))
            .field("buffer", &format!("<buffer capacity: {}>", self.capacity()))
            .finish()
    }
}

impl<T> ops::Index<usize> for RingBuffer<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        let pos = self.get_pos_from_index(index);
        unsafe { (&*self.buffer[pos].get()).assume_init_ref() }
    }
}

impl<T> ops::IndexMut<usize> for RingBuffer<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let pos = self.get_pos_from_index(index);
        unsafe { self.buffer[pos].get_mut().assume_init_mut() }
    }
}

impl<T> RingBuffer<T> {
    /// Creates a new ring buffer with the given capacity.
    /// Capacity will be rounded up to the next power of 2.
    pub fn new(mut capacity: usize) -> Self {
        // Ensure capacity is a power of 2
        capacity = capacity.next_power_of_two();

        RingBuffer {
            read_index: CachePadded::new(AtomicUsize::new(0)),
            read_index_cached: ThreadLocal::new(),
            write_index: CachePadded::new(AtomicUsize::new(0)),
            write_index_cached: ThreadLocal::new(),
            mask: capacity - 1,
            buffer: std::iter::repeat_with(|| UnsafeCell::new(MaybeUninit::uninit()))
                .take(capacity)
                .collect(),
        }
    }

    /// Returns the capacity of the ring buffer
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Returns the number of elements in the buffer
    ///
    /// This method attempts to use thread-local cached indices first to avoid
    /// cache coherency traffic. It falls back to atomic loads if cached values
    /// aren't available.
    pub fn len(&self) -> usize {
        self.write_index().wrapping_sub(self.read_index())
    }

    pub fn free_slots(&self) -> usize {
        self.capacity() - self.len()
    }

    /// Returns true if the buffer is empty
    ///
    /// This method uses thread-local cached indices where available
    /// to avoid cache coherency traffic.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns true if the buffer is full
    ///
    /// This method uses thread-local cached indices where available
    /// to avoid cache coherency traffic. If you're checking before pushing,
    /// it's more efficient to just call `push` directly and handle the Result.
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    /// Attempts to push an item to the buffer.
    /// Returns Ok(()) if successful, or Err(item) if the buffer is full,
    /// returning the original item.
    pub fn push(&self, item: T) -> Result<(), T> {
        if self.is_full() {
            self.reload_indices();
            if self.is_full() {
                return Err(item);
            }
        }

        let write = self.write_index();
        // Write the item to the buffer
        unsafe {
            let cell = &self.buffer[write & self.mask];
            (*cell.get()).write(item);
        }

        // Update write index with release semantics
        self.update_write_index(write.wrapping_add(1));
        Ok(())
    }

    pub fn push_slice(&self, slice: &[T]) -> usize
    where
        T: Copy,
    {
        self.reload_indices();
        let mut write = self.write_index();
        let available = self.free_slots().min(slice.len());
        for x in &slice[..available] {
            unsafe {
                let ptr = &self.buffer[write & self.mask];
                (*ptr.get()).write(*x);
                write = write.wrapping_add(1);
            }
        }
        self.update_write_index(write);
        available
    }

    /// Attempts to pop an item from the buffer.
    /// Returns Some(item) if successful, None if the buffer is empty.
    pub fn pop(&self) -> Option<T> {
        if self.is_empty() {
            self.reload_indices();
            if self.is_empty() {
                return None;
            }
        }

        let read = self.read_index();

        // Read the item from the buffer
        let item = unsafe {
            let cell = &self.buffer[read & self.mask];
            (*cell.get()).assume_init_read()
        };

        // Update read index with release semantics
        self.update_read_index(read.wrapping_add(1));
        Some(item)
    }

    pub fn pop_slice(&self, slice: &mut [T]) -> usize {
        self.reload_indices();
        let mut read = self.read_index();
        let available = self.len().min(slice.len());
        for x in &mut slice[..available] {
            unsafe {
                let ptr = &self.buffer[read & self.mask];
                *x = (*ptr.get()).assume_init_read();
                read = read.wrapping_add(1);
            }
        }
        self.update_read_index(read);
        available
    }

    pub fn drop_items(&self, amount: usize) -> usize {
        self.reload_indices();
        let amount = amount.min(self.len());
        let mut read = self.read_index();
        for _ in 0..amount {
            let ptr = self.buffer[read & self.mask].get();
            unsafe {
                (&mut *ptr).assume_init_drop();
            }
            read = read.wrapping_add(1);
        }
        self.update_read_index(read);
        amount
    }

    pub fn read_pos(&self) -> usize {
        self.read_index() & self.mask
    }

    pub fn write_pos(&self) -> usize {
        self.write_index() & self.mask
    }

    pub(crate) fn reload_indices(&self) {
        if let Some(read_cell) = self.read_index_cached.get() {
            read_cell.set(self.read_index.load(Ordering::SeqCst));
        }
        if let Some(write_cell) = self.write_index_cached.get() {
            write_cell.set(self.write_index.load(Ordering::SeqCst));
        }
    }

    fn read_index(&self) -> usize {
        self.read_index_cached
            .get_or(|| Cell::new(self.read_index.load(Ordering::SeqCst)))
            .get()
    }

    fn update_read_index(&self, index: usize) {
        if let Some(cell) = self.read_index_cached.get() {
            cell.set(index);
        }
        self.read_index.store(index, Ordering::SeqCst);
    }

    fn write_index(&self) -> usize {
        self.write_index_cached
            .get_or(|| Cell::new(self.write_index.load(Ordering::SeqCst)))
            .get()
    }

    fn update_write_index(&self, index: usize) {
        if let Some(cell) = self.write_index_cached.get() {
            cell.set(index);
        }
        self.write_index.store(index, Ordering::SeqCst);
    }

    fn get_pos_from_index(&self, index: usize) -> usize {
        assert!(
            index < self.len(),
            "Index out of bounds (index {index} >= len {len})",
            len = self.len()
        );
        self.read_index().wrapping_add(index) & self.mask
    }
}

impl<T> Drop for RingBuffer<T> {
    fn drop(&mut self) {
        // Destroy any items still in the buffer
        while let Some(_) = self.pop() {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn test_new_capacity() {
        let rb = RingBuffer::<i32>::new(10);
        assert_eq!(rb.capacity(), 16); // Rounds up to the next power of 2

        let rb = RingBuffer::<i32>::new(16);
        assert_eq!(rb.capacity(), 16);

        let rb = RingBuffer::<i32>::new(0);
        assert_eq!(rb.capacity(), 1); // Minimum capacity is 1 (2^0)
    }

    #[test]
    fn test_empty_full() {
        let rb = RingBuffer::<i32>::new(4);
        assert!(rb.is_empty());
        assert!(!rb.is_full());
        assert_eq!(rb.len(), 0);

        rb.push(1).unwrap();
        assert!(!rb.is_empty());
        assert!(!rb.is_full());
        assert_eq!(rb.len(), 1);

        rb.push(2).unwrap();
        rb.push(3).unwrap();
        rb.push(4).unwrap();
        assert!(!rb.is_empty());
        assert!(rb.is_full());
        assert_eq!(rb.len(), 4);

        // Buffer is full, this should fail
        assert!(rb.push(5).is_err());
        assert_eq!(rb.len(), 4);
    }

    #[test]
    fn test_push_pop() {
        let rb = RingBuffer::<i32>::new(4);

        rb.push(1).unwrap();
        rb.push(2).unwrap();
        rb.push(3).unwrap();

        assert_eq!(rb.pop(), Some(1));
        assert_eq!(rb.pop(), Some(2));
        assert_eq!(rb.pop(), Some(3));
        assert_eq!(rb.pop(), None); // Buffer is empty

        // Check that we can reuse the buffer
        rb.push(4).unwrap();
        rb.push(5).unwrap();
        assert_eq!(rb.pop(), Some(4));
        assert_eq!(rb.pop(), Some(5));
    }

    #[test]
    fn test_wrapping_behavior() {
        let rb = RingBuffer::<i32>::new(4);

        // Fill the buffer
        for i in 1..=4 {
            rb.push(i).unwrap();
        }

        // Remove two items
        assert_eq!(rb.pop(), Some(1));
        assert_eq!(rb.pop(), Some(2));

        // Add two more (should wrap around in the internal buffer)
        rb.push(5).unwrap();
        rb.push(6).unwrap();

        // Check that all items are retrieved in order
        assert_eq!(rb.pop(), Some(3));
        assert_eq!(rb.pop(), Some(4));
        assert_eq!(rb.pop(), Some(5));
        assert_eq!(rb.pop(), Some(6));
        assert_eq!(rb.pop(), None);
    }

    #[test]
    fn test_reload_indices() {
        let rb = RingBuffer::<i32>::new(4);

        // This will initialize thread-local caches
        assert!(rb.is_empty());

        // Manually update the atomic indices (simulating another thread's changes)
        rb.read_index.store(1, Ordering::SeqCst);
        rb.write_index.store(3, Ordering::SeqCst);

        // Local cached values are still at 0, so len() would return 0
        // But after reload_indices it should reflect the new values
        rb.reload_indices();
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn test_push_pop_slice() {
        let rb = RingBuffer::<i32>::new(8);

        // Test push_slice
        let data = [1, 2, 3, 4, 5];
        let pushed = rb.push_slice(&data);
        assert_eq!(pushed, 5);

        // Try to push more than capacity
        let data2 = [6, 7, 8, 9, 10, 11, 12];
        let pushed = rb.push_slice(&data2);
        assert_eq!(pushed, 3); // Only 3 more should fit

        // Test pop_slice
        let mut result = [0; 4];
        let popped = rb.pop_slice(&mut result);
        assert_eq!(popped, 4);
        assert_eq!(result, [1, 2, 3, 4]);

        // Pop the rest
        let mut result2 = [0; 10];
        let popped = rb.pop_slice(&mut result2);
        assert_eq!(popped, 4);
        assert_eq!(result2[0..4], [5, 6, 7, 8]);

        // Buffer should be empty now
        assert!(rb.is_empty());
    }

    #[test]
    fn test_index_wrapping() {
        let rb = RingBuffer::<i32>::new(4);

        // Fill and drain many times to force index wrapping
        for cycle in 0..10 {
            for i in 0..4 {
                rb.push(i + cycle * 10).unwrap();
            }

            for i in 0..4 {
                assert_eq!(rb.pop(), Some(i + cycle * 10));
            }
        }

        // Check that the indices have wrapped around multiple times
        // but the buffer still works correctly
        rb.push(100).unwrap();
        assert_eq!(rb.pop(), Some(100));
    }

    #[test]
    fn test_drop_cleanup() {
        #[derive(Debug)]
        struct DropCounter {
            counter: Arc<AtomicUsize>,
        }

        impl Drop for DropCounter {
            fn drop(&mut self) {
                self.counter.fetch_add(1, Ordering::SeqCst);
            }
        }

        let counter = Arc::new(AtomicUsize::new(0));

        {
            let rb = RingBuffer::<DropCounter>::new(4);

            // Add 3 items
            for _ in 0..3 {
                rb.push(DropCounter {
                    counter: Arc::clone(&counter),
                })
                .expect("Ring buffer should not be full");
            }

            // At this point, nothing should have been dropped
            assert_eq!(counter.load(Ordering::SeqCst), 0);
        }

        // After the RingBuffer is dropped, all 3 items should be dropped
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_multithreaded_usage() {
        let rb = Arc::new(RingBuffer::<i32>::new(1024));
        let values = Arc::new(Mutex::new(Vec::new()));

        // Spawn a producer thread
        let rb_producer = Arc::clone(&rb);
        let producer = thread::spawn(move || {
            for i in 0..100 {
                rb_producer
                    .push(i)
                    .expect("Push failed even through it should not have been at capacity");
            }
        });

        // Spawn a consumer thread
        let rb_consumer = Arc::clone(&rb);
        let values_consumer = Arc::clone(&values);
        let consumer = thread::spawn(move || {
            let mut local_values = Vec::new();

            for _ in 0..100 {
                loop {
                    if let Some(val) = rb_consumer.pop() {
                        local_values.push(val);
                        break;
                    } else {
                        thread::yield_now();
                    }
                }
            }

            let mut values = values_consumer.lock().unwrap();
            values.extend(local_values);
        });

        // Wait for both threads to complete
        producer.join().unwrap();
        consumer.join().unwrap();

        // Verify we got all values
        let values = values.lock().unwrap();
        assert_eq!(values.len(), 100);

        // Check that all produced values were consumed
        let mut counts = std::collections::HashMap::new();
        for &val in values.iter() {
            *counts.entry(val).or_insert(0) += 1;
        }

        for i in 0..100 {
            let count = counts.get(&i);
            assert!(count.is_some(), "{i} not counted");
            assert_eq!(count.copied().unwrap(), 1);
        }
    }

    #[test]
    fn test_multithreaded_overflow() {
        let rb = Arc::new(RingBuffer::<i32>::new(100));
        let values = Arc::new(Mutex::new(Vec::new()));
        let overridden = Arc::new(AtomicUsize::new(0));

        // Spawn a producer thread
        let rb_producer = Arc::clone(&rb);
        let overridden_producer = Arc::clone(&overridden);
        let producer = thread::spawn(move || {
            for i in 0..200 {
                if let Err(i) = rb_producer.push(i) {
                    rb_producer.pop().expect("Ring buffer should not be empty");
                    overridden_producer.fetch_add(1, Ordering::SeqCst);
                    rb_producer
                        .push(i)
                        .expect("Push failed even through it should not have been at capacity");
                }
            }
        });

        // Spawn a consumer thread
        let rb_consumer = Arc::clone(&rb);
        let values_consumer = Arc::clone(&values);
        let consumer = thread::spawn(move || {
            let mut local_values = Vec::new();

            for _ in 0..100 {
                loop {
                    if let Some(val) = rb_consumer.pop() {
                        local_values.push(val);
                        break;
                    } else {
                        thread::yield_now();
                    }
                }
            }

            let mut values = values_consumer.lock().unwrap();
            values.extend(local_values);
        });

        // Wait for both threads to complete
        producer.join().unwrap();
        consumer.join().unwrap();

        // Verify we got all values
        let values = values.lock().unwrap();
        assert_eq!(values.len(), 100);

        // Check that all produced values were consumed
        let mut counts = std::collections::HashMap::new();
        for &val in values.iter() {
            *counts.entry(val).or_insert(0) += 1;
        }

        assert_eq!(counts.len(), 100);
        assert!(overridden.load(Ordering::SeqCst) > 0, "No values were overridden");
    }

    #[test]
    fn test_update_read_write_indices() {
        let rb = RingBuffer::<i32>::new(4);

        // Check initial state
        assert_eq!(rb.read_pos(), 0);
        assert_eq!(rb.write_pos(), 0);

        // Update indices
        rb.update_read_index(2);
        rb.update_write_index(3);

        // Check updated values
        assert_eq!(rb.read_pos(), 2);
        assert_eq!(rb.write_pos(), 3);

        // Check atomic values directly
        assert_eq!(rb.read_index.load(Ordering::SeqCst), 2);
        assert_eq!(rb.write_index.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_index_access() {
        let rb = RingBuffer::<i32>::new(4);

        // Test normal case
        rb.push(1).unwrap();
        rb.push(2).unwrap();
        rb.push(3).unwrap();

        assert_eq!(rb[0], 1); // Oldest value
        assert_eq!(rb[1], 2);
        assert_eq!(rb[2], 3); // Newest value

        // Test wrapping case
        rb.pop();
        rb.pop(); // Remove 1,2
        rb.push(4).unwrap();
        rb.push(5).unwrap();

        assert_eq!(rb[0], 3); // Oldest
        assert_eq!(rb[1], 4);
        assert_eq!(rb[2], 5); // Newest

        // Test out of bounds
        std::panic::catch_unwind(|| {
            let _v = rb[3]; // Should panic
        })
        .expect_err("Index past length should panic");
    }

    #[test]
    #[should_panic]
    fn test_index_access_empty() {
        let rb = RingBuffer::<i32>::new(4);
        let _v = rb[0];
    }
}
