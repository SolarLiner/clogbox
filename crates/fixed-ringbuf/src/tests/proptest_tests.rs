#![cfg(not(miri))]
use crate::rb::RingBuffer;
use proptest::prelude::*;

proptest! {
    #[test]
    fn proptest_push_pop_preserves_values(values in prop::collection::vec(0..100i32, 1..100)) {
        let rb = RingBuffer::new(values.len());

        // Push all values
        for &val in &values {
            assert!(rb.push(val).is_ok());
        }

        // Pop all values and check they match
        for &expected in &values {
            let popped = rb.pop();
            assert_eq!(popped, Some(expected));
        }

        // Should be empty now
        assert_eq!(rb.pop(), None);
    }

    #[test]
    fn proptest_capacity_always_power_of_two(cap in 0..1000usize) {
        let rb = RingBuffer::<i32>::new(cap);
        let capacity = rb.capacity();

        // Capacity should be at least 1
        assert!(capacity >= 1);

        // Capacity should be power of two
        assert_eq!(capacity.count_ones(), 1);

        // Capacity should be >= the requested capacity
        assert!(capacity >= cap);

        // Capacity should be the smallest power of two >= cap
        if cap > 0 {
            assert!(capacity < cap * 2);
        }
    }

    #[test]
    fn proptest_push_slice_pop_slice(values in prop::collection::vec(0..100i32, 1..100)) {
        let rb = RingBuffer::new(values.len());

        // Push all values as a slice
        let pushed = rb.push_slice(&values);
        assert_eq!(pushed, values.len());

        // Pop as a slice
        let mut result = vec![0; values.len()];
        let popped = rb.pop_slice(&mut result);
        assert_eq!(popped, values.len());

        // Check popped values match
        for i in 0..values.len() {
            assert_eq!(result[i], values[i]);
        }
    }

    #[test]
    fn proptest_push_len(values in prop::collection::vec(0..100i32, 1..100)) {
        let rb = RingBuffer::new(values.len());
        for &val in &values {
            rb.push(val).unwrap();
        }
        assert_eq!(values.len(), rb.len());

        for &val in &values {
            let actual = rb.pop().unwrap();
            assert_eq!(val, actual);
        }
        assert_eq!(0, rb.len());
    }

    #[test]
    fn proptest_push_until_full(values in prop::collection::vec(0..100i32, 1..200)) {
        let capacity = values.len();
        let rb = RingBuffer::new(capacity);

        // Push until no more room
        let mut pushed_count = 0;
        for &val in &values {
            if rb.push(val).is_ok() {
                pushed_count += 1;
            } else {
                break;
            }
        }

        // Should have pushed exactly capacity items
        assert_eq!(pushed_count, capacity);

        // Pop everything
        for i in 0..pushed_count {
            assert_eq!(rb.pop(), Some(values[i]));
        }

        // Should be empty now
        assert!(rb.is_empty());
    }

    #[test]
    fn proptest_wraparound_behavior(
        values in prop::collection::vec(0..100i32, 50..150),
        iterations in 1..10u32
    ) {
        let capacity = 32;
        let rb = RingBuffer::new(capacity);

        for _ in 0..iterations {
            // Fill buffer completely
            for i in 0..rb.capacity() {
                rb.push(i as i32).unwrap();
            }

            // Remove half
            for i in 0..rb.capacity()/2 {
                assert_eq!(rb.pop(), Some(i as i32));
            }

            // Fill the gap (causes wraparound)
            for i in 0..rb.capacity()/2 {
                rb.push((rb.capacity() + i) as i32).unwrap();
            }

            // Empty buffer and verify values
            for i in 0..rb.capacity() {
                let expected = if i < rb.capacity()/2 {
                    (rb.capacity()/2 + i) as i32
                } else {
                    (rb.capacity() + i - rb.capacity()/2) as i32
                };
                assert_eq!(rb.pop(), Some(expected));
            }

            // Should be empty now
            assert!(rb.is_empty());
        }
    }
}
