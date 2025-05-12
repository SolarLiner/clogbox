mod miri_tests;
mod proptest_tests;
use super::*;

#[test]
fn test_producer_push() {
    let (producer, _) = create(2);
    assert!(producer.push(1).is_ok());
    assert!(producer.push(2).is_ok());
    assert!(producer.push(3).is_err());
}

#[test]
fn test_producer_push_slice() {
    let (producer, _) = create(4);
    let items = [1, 2, 3, 4];
    assert_eq!(producer.push_slice(&items), 4);
    assert_eq!(producer.push_slice(&[5, 6]), 0);
}

#[test]
fn test_producer_push_overriding() {
    let (producer, _) = create(2);
    assert_eq!(producer.push_overriding(1), None);
    assert_eq!(producer.push_overriding(2), None);
    assert_eq!(producer.push_overriding(3), Some(1));
}

#[test]
fn test_producer_push_slice_overriding() {
    let (producer, _) = create(2);
    producer.push_slice_overriding(&[1, 2]);
    producer.push_slice_overriding(&[3, 4]);
    let mut result = vec![0; 2];
    let consumer = Consumer {
        rb: producer.rb.clone(),
        _marker: PhantomData,
    };
    assert_eq!(consumer.pop_slice(&mut result), 2);
    assert_eq!(result, vec![3, 4]);
}

#[test]
fn test_consumer_pop() {
    let (producer, consumer) = create(2);
    producer.push(1).unwrap();
    producer.push(2).unwrap();
    assert_eq!(consumer.pop(), Some(1));
    assert_eq!(consumer.pop(), Some(2));
    assert_eq!(consumer.pop(), None);
}

#[test]
fn test_consumer_pop_slice() {
    let (producer, consumer) = create(4);
    producer.push_slice(&[1, 2, 3, 4]);
    let mut result = vec![0; 2];
    assert_eq!(consumer.pop_slice(&mut result), 2);
    assert_eq!(result, vec![1, 2]);
}

#[test]
fn test_producer_consumer_interaction() {
    let (producer, consumer) = create(4);
    producer.push_slice(&[1, 2]);
    assert_eq!(consumer.pop(), Some(1));
    producer.push_slice(&[3, 4]);
    let mut result = vec![0; 3];
    assert_eq!(consumer.pop_slice(&mut result), 3);
    assert_eq!(result[0..3], vec![2, 3, 4]);
}
