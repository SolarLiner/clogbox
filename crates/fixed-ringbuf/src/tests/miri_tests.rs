use crate::rb::RingBuffer;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

// These tests are specifically designed to be run with MIRI to detect
// undefined behavior in unsafe code.

#[test]
fn miri_basic_push_pop() {
    // Basic test for the unsafe buffer access
    let rb = RingBuffer::<i32>::new(4);

    rb.push(1).unwrap();
    rb.push(2).unwrap();

    assert_eq!(rb.pop(), Some(1));
    assert_eq!(rb.pop(), Some(2));
    assert_eq!(rb.pop(), None);
}

#[test]
fn miri_buffer_wrapping() {
    // Test buffer wrapping behavior with unsafe reads/writes
    let rb = RingBuffer::<i32>::new(4);

    // Fill the buffer
    for i in 1..=4 {
        rb.push(i).unwrap();
    }

    // Pop the first two elements
    assert_eq!(rb.pop(), Some(1));
    assert_eq!(rb.pop(), Some(2));

    // Push two more (will wrap around in the buffer)
    rb.push(5).unwrap();
    rb.push(6).unwrap();

    // Pop remaining elements
    assert_eq!(rb.pop(), Some(3));
    assert_eq!(rb.pop(), Some(4));
    assert_eq!(rb.pop(), Some(5));
    assert_eq!(rb.pop(), Some(6));
}

#[test]
fn miri_push_pop_large_type() {
    // Test with a large type to ensure proper memory handling
    #[derive(Debug, PartialEq)]
    struct LargeType {
        data: [u8; 64],
        value: i32,
    }

    let rb = RingBuffer::<LargeType>::new(4);

    let item1 = LargeType {
        data: [1; 64],
        value: 42,
    };
    let item2 = LargeType {
        data: [2; 64],
        value: 43,
    };

    rb.push(item1).unwrap();
    rb.push(item2).unwrap();

    // Check that we get the same values back
    let popped1 = rb.pop().unwrap();
    assert_eq!(popped1.data[0], 1);
    assert_eq!(popped1.value, 42);

    let popped2 = rb.pop().unwrap();
    assert_eq!(popped2.data[0], 2);
    assert_eq!(popped2.value, 43);
}

#[test]
fn miri_push_pop_slice() {
    // Test slice operations that involve multiple unsafe accesses
    let rb = RingBuffer::<i32>::new(8);

    let data = [1, 2, 3, 4, 5];
    let pushed = rb.push_slice(&data);
    assert_eq!(pushed, 5);

    let mut result = [0; 3];
    let popped = rb.pop_slice(&mut result);
    assert_eq!(popped, 3);
    assert_eq!(result, [1, 2, 3]);

    let mut result2 = [0; 3];
    let popped = rb.pop_slice(&mut result2);
    assert_eq!(popped, 2);
    assert_eq!(result2[0..2], [4, 5]);
}

#[test]
fn miri_drop_behavior() {
    // Test proper cleanup in `std::ops::Drop` with a type that tracks drops
    #[derive(Debug)]
    struct DropTracker {
        counter: Arc<AtomicUsize>,
        id: usize,
    }

    impl Drop for DropTracker {
        fn drop(&mut self) {
            self.counter.fetch_add(1, Ordering::SeqCst);
        }
    }

    let counter = Arc::new(AtomicUsize::new(0));

    {
        let rb = RingBuffer::<DropTracker>::new(4);

        for i in 0..3 {
            rb.push(DropTracker {
                counter: counter.clone(),
                id: i,
            })
            .unwrap();
        }

        // Pop one item to test mixed behavior
        rb.pop();

        // Counter should be 1 now (one item popped)
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Let rb go out of scope, which should drop the remaining items
    }

    // Counter should be 3 now (all items dropped)
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[test]
fn miri_multithreaded_push_pop() {
    // Test multithreaded safety with a small number of operations
    // (MIRI runs extremely slowly with threads, so we keep this minimal)
    let rb = Arc::new(RingBuffer::<i32>::new(16));

    // Spawn producer thread
    let rb_producer = Arc::clone(&rb);
    let producer = thread::spawn(move || {
        for i in 0..5 {
            while rb_producer.push(i).is_err() {
                // Buffer full, try again
                thread::yield_now();
            }
        }
    });

    // Spawn consumer thread
    let rb_consumer = Arc::clone(&rb);
    let consumer = thread::spawn(move || {
        let mut values = Vec::new();
        for _ in 0..5 {
            loop {
                if let Some(val) = rb_consumer.pop() {
                    values.push(val);
                    break;
                }
                thread::yield_now();
            }
        }
        values
    });

    // Wait for threads to complete
    producer.join().unwrap();
    let values = consumer.join().unwrap();

    // Check that we got 5 values
    assert_eq!(values.len(), 5);

    // Values should be in order since we only had one producer
    for i in 0..5 {
        assert_eq!(values[i], i as i32);
    }
}
