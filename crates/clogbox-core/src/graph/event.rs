use crate::graph::Timestamped;
use std::cmp::Ordering;
use std::fmt;
use std::fmt::Formatter;
use std::iter::Peekable;

/// Fixed-sized event buffer type, which stores timestamped events in a sorted array.
#[derive(Clone)]
pub struct EventBuffer<T> {
    data: Vec<Timestamped<T>>,
}

impl<T> fmt::Debug for EventBuffer<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventBuffer").finish_non_exhaustive()
    }
}

impl<T> EventBuffer<T> {
    /// Creates a new event buffer with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Iterate over all timestamped events.
    pub fn iter_events(&self) -> impl Iterator<Item = Timestamped<&T>> {
        self.data.iter().map(|t| t.as_ref())
    }

    /// Mutably iterate over all timestamped events.
    pub fn iter_events_mut(&mut self) -> impl Iterator<Item = Timestamped<&mut T>> {
        self.data.iter_mut().map(|t| t.as_mut())
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }
}

impl<T: PartialOrd> EventBuffer<T> {
    /// Push an event at the given timestamp, if capacity allows.
    ///
    /// Otherwise, returns the passed in event.
    ///
    /// # Arguments
    ///
    /// - `timestamp`: Timestamp at which to store the event.
    /// - `event`: Event to store.
    pub fn push(&mut self, timestamp: usize, event: T) -> Result<(), T> {
        if self.len() == self.capacity() {
            Err(event)
        } else {
            let entry = Timestamped {
                sample: timestamp,
                value: event,
            };
            match self.data.binary_search_by(|v| {
                v.sample
                    .cmp(&timestamp)
                    .then_with(|| v.value.partial_cmp(&entry.value).unwrap_or(Ordering::Equal))
            }) {
                Ok(pos) => {
                    // Exact same event found at index `pos`, overwriting
                    self.data[pos] = entry;
                }
                Err(pos) => {
                    // Insert new event at index `pos` to keep ordering
                    self.data.insert(pos, entry);
                }
            }
            Ok(())
        }
    }

    /// Returns the next event for the given timestamp, if there is one.
    ///
    /// # Arguments
    ///
    /// - `timestamp`: Timestamp to query for
    pub fn next_event(&self, timestamp: usize) -> Option<Timestamped<&T>> {
        match self.data.binary_search_by_key(&timestamp, |t| t.sample) {
            Ok(pos) => Some(self.data[pos].as_ref()),
            Err(pos) => {
                let event = self.data[pos].as_ref();
                if event.sample > timestamp {
                    Some(event)
                } else {
                    None
                }
            }
        }
    }
}

/// Iterator combinator for iterators of timestamped values, emitting as an ordered iterator of
/// the underlying timestamped values
pub struct OrderedTimestampCombinator<I: Iterator, J: Iterator<Item = I::Item>> {
    iter1: Peekable<I>,
    iter2: Peekable<J>,
}

impl<T: Ord, I: Iterator<Item = Timestamped<T>>, J: Iterator<Item = I::Item>> Iterator
    for OrderedTimestampCombinator<I, J>
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.iter1.peek(), self.iter2.peek()) {
            (Some(a), Some(b)) => {
                if a.cmp(b).is_lt() {
                    self.iter1.next()
                } else {
                    self.iter2.next()
                }
            }
            (Some(_), None) => self.iter1.next(),
            (None, Some(_)) => self.iter2.next(),
            (None, None) => None,
        }
    }
}

pub trait TimestampedIteratorExt<T>: Sized + Iterator<Item = Timestamped<T>> {
    /// Consumes both iterators, returning the merged iterator, keeping ordering
    fn ordered_with<I: IntoIterator<Item = Timestamped<T>>>(
        self,
        other: I,
    ) -> OrderedTimestampCombinator<Self, I::IntoIter> {
        OrderedTimestampCombinator {
            iter1: self.peekable(),
            iter2: other.into_iter().peekable(),
        }
    }
}

impl<T, I: Iterator<Item = Timestamped<T>>> TimestampedIteratorExt<T> for I {}
