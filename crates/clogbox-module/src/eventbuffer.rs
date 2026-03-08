//! # Event buffer implementation for parameter and note events
//!
//! This module provides a buffer structure for storing and processing time-stamped events
//! such as parameter changes and MIDI notes.

use std::mem::MaybeUninit;
use std::ops::{Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive};
use std::{mem, ops, ptr, slice};

/// A wrapper for data with an associated timestamp.
///
/// Provides a container that pairs data with a numeric timestamp for chronological ordering.
#[derive(Debug, Copy, Clone)]
pub struct Timestamped<T: ?Sized> {
    /// The timestamp of the event.
    pub timestamp: usize,
    /// The event data.
    pub data: T,
}

impl<T> ops::Deref for Timestamped<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> ops::DerefMut for Timestamped<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<T> PartialEq for Timestamped<T> {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp.eq(&other.timestamp)
    }
}

impl<T> Eq for Timestamped<T> {}

impl<T> PartialOrd for Timestamped<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Timestamped<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

impl<T> Timestamped<T> {
    /// Map the inner value of this [`Timestamped`].
    #[inline]
    pub fn map<U>(self, func: impl FnOnce(T) -> U) -> Timestamped<U> {
        let Self { timestamp, data } = self;
        Timestamped {
            timestamp,
            data: func(data),
        }
    }

    /// Maps the inner value of this [`Timestamped`], returning `None` if the mapping function returns `None`.
    #[inline]
    pub fn filter_map<U>(self, func: impl FnOnce(T) -> Option<U>) -> Option<Timestamped<U>> {
        let Self { timestamp, data } = self;
        Some(Timestamped {
            timestamp,
            data: func(data)?,
        })
    }

    /// Return this [`Timestamped`] as a reference to the inner value, copying the timestamp.
    #[inline]
    pub fn as_ref(&self) -> Timestamped<&T> {
        let Self { timestamp, ref data } = *self;
        Timestamped { timestamp, data }
    }

    /// Return this [`Timestamped`] as a mutable reference to the inner value, copying the timestamp.
    #[inline]
    pub fn as_mut(&mut self) -> Timestamped<&mut T> {
        let Self {
            timestamp,
            ref mut data,
        } = *self;
        Timestamped { timestamp, data }
    }
}

impl<T> Timestamped<Option<T>> {
    /// "Transposes" an `Timestamped<Option<T>>` into an `Option<Timestamped<T>>`.
    #[inline]
    pub fn transpose(self) -> Option<Timestamped<T>> {
        self.filter_map(std::convert::identity)
    }
}

impl<T, E> Timestamped<Result<T, E>> {
    /// "Transposes" an `Timestamped<Result<T, E>>` into a `Result<Timestamped<T>, E>`.
    #[inline]
    pub fn transpose(self) -> Result<Timestamped<T>, E> {
        let Self { timestamp, data } = self;
        Ok(Timestamped { timestamp, data: data? })
    }
}

/// A mutable iterator that sorts the buffer again when dropped.
///
/// This iterator allows modifying [`Timestamped`] entries in an [`EventBuffer`].
/// When the iterator goes out of scope (is dropped), the underlying buffer is
/// automatically re-sorted by timestamp to maintain chronological order.
pub struct IterMut<'a, T> {
    events: &'a mut [Timestamped<T>],
    index: usize,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut Timestamped<T>;

    /// Returns the next mutable reference to a [`Timestamped<T>`] entry.
    ///
    /// Internally uses a lifetime manipulation technique to enable multiple mutable
    /// references to different elements of the same vector.
    fn next(&mut self) -> Option<Self::Item> {
        // Mangle lifetime of the buffer.
        // SAFETY: the lifetime is tied to the IterMut lifetime and each item is yielded only once.
        let events = unsafe { mem::transmute::<&mut [Timestamped<T>], &mut [Timestamped<T>]>(self.events) };

        if self.index < events.len() {
            let item = &mut events[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

impl<T> Drop for IterMut<'_, T> {
    /// Automatically re-sorts the underlying buffer when the iterator is dropped.
    ///
    /// This ensures chronological ordering after timestamp modifications.
    fn drop(&mut self) {
        // Re-sort the events when the iterator is dropped
        self.events.sort();
    }
}

/// Trait of types which are collections of timestamped events.
pub trait TimestampedCollection<T> {
    /// Returns a slice of all events contained in this collection
    fn slice(&self) -> &EventSlice<T>;
}

/// Trait of types which are mutable collections of timestamped events.
pub trait TimestampedCollectionMut<T>: TimestampedCollection<T> {
    /// Returns a mutable slice of all events contained in this collection
    fn slice_mut(&mut self) -> &mut EventSlice<T>;

    /// Push a new event into this collection. Returns false if the event could not be pushed.
    fn push(&mut self, timestamp: usize, event: T) -> bool;
}

/// A non-owned view into a sequence of timestamped events.
///
/// This is to [`EventBuffer`] what `[T]` is to `Vec<T>`. It provides a borrowed
/// view into a sequence of [`Timestamped<T>`] events with various methods for
/// working with time-based data.
///
/// The `#[repr(transparent)]` attribute ensures memory layout compatibility with
/// the underlying slice, making transmutation in [`from_slice`](Self::from_slice) safe.
#[derive(Debug)]
#[repr(transparent)]
pub struct EventSlice<T> {
    /// The slice of timestamped events this reference points to.
    events: [Timestamped<T>],
}

impl<'a, T> IntoIterator for &'a EventSlice<T> {
    type Item = &'a Timestamped<T>;
    type IntoIter = slice::Iter<'a, Timestamped<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut EventSlice<T> {
    type Item = &'a mut Timestamped<T>;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        IterMut {
            index: 0,
            events: &mut self.events,
        }
    }
}

impl<T> EventSlice<T> {
    /// Creates a reference to an [`EventSlice`] from a slice of timestamped events.
    ///
    /// This conversion is safe because [`EventSlice<T>`] is marked with `#[repr(transparent)]`,
    /// guaranteeing that its memory layout is identical to `[Timestamped<T>]`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventSlice, Timestamped};
    /// let events = vec![Timestamped { timestamp: 1, data: "a" }];
    /// let slice = EventSlice::from_slice(&events);
    /// ```
    pub fn from_slice(events: &[Timestamped<T>]) -> &Self {
        assert!(
            events.is_sorted_by_key(|e| e.timestamp),
            "Events slice needs to be sorted by timestamp"
        );
        // Safety: The memory layout of `EventSlice<T>` is identical to `[Timestamped<T>]`
        // due to the #[repr(transparent)] attribute
        unsafe { mem::transmute::<&[Timestamped<T>], &Self>(events) }
    }

    /// Creates a reference to an [`EventSlice`] from a mutable slice of timestamped events.
    ///
    /// This conversion is safe because [`EventSlice<T>`] is marked with `#[repr(transparent)]`,
    /// guaranteeing that its memory layout is identical to `[Timestamped<T>]`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventSlice, Timestamped};
    /// let mut events = vec![Timestamped { timestamp: 1, data: "a" }];
    /// let slice = EventSlice::from_mut_slice(&mut events);
    /// slice[0].data = "b";
    /// assert_eq!("b", events[0].data); // Mutated the original slice
    /// ```
    pub fn from_mut_slice(events: &mut [Timestamped<T>]) -> &mut Self {
        events.sort();
        // Safety: The memory layout of `EventSlice<T>` is identical to `[Timestamped<T>]`
        // due to the #[repr(transparent)] attribute
        unsafe { mem::transmute::<&mut [Timestamped<T>], &mut Self>(events) }
    }

    /// Returns the number of events in the slice.
    ///
    /// This is equivalent to the [`len`](slice::len) method on slices.
    pub const fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns `true` if the slice contains no events.
    ///
    /// This is equivalent to the [`is_empty`](slice::is_empty) method on slices.
    pub const fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns a reference to the value at the specified index if it exists. Note that this is **not** returning the
    /// value by its timestamp, use [`at`](Self:at) for this purpose.
    ///
    /// Returns `None` if the index is out of bounds.
    pub fn get(&self, index: usize) -> Option<Timestamped<&T>> {
        self.events.get(index).map(Timestamped::as_ref)
    }

    /// Return a slice of all events at the given timestamp.
    ///
    /// # Arguments
    ///
    /// * `timestamp`: Timestamp to retrieve the value for.
    pub fn at(&self, timestamp: usize) -> &Self {
        self.slice_by_timestamp(self.range_for_timestamp(timestamp))
    }

    /// Return a mutable slice of all events at the given timestamp.
    ///
    /// # Arguments
    ///
    /// * `timestamp`: Timestamp to retrieve the value for.
    pub fn at_mut(&mut self, timestamp: usize) -> &mut Self {
        let range = self.range_for_timestamp(timestamp);
        self.slice_mut(range)
    }

    /// Returns an iterator over the events.
    ///
    /// Events are yielded in chronological order (by timestamp).
    pub fn iter(&self) -> impl Iterator<Item = &Timestamped<T>> {
        self.events.iter()
    }

    /// Returns a mutable iterator over the events.
    ///
    /// Events are yielded in chronological order (by timestamp).
    /// The event slice will be reordered after this iterator is consumed, such that any modifications to the timestamps
    /// will not violate the invariant that elements must be ordered by their timestamps.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Timestamped<T>> {
        IterMut {
            events: &mut self.events,
            index: 0,
        }
    }

    /// Returns a subslice of this [`EventSlice`] based on the provided timestamp range.
    ///
    /// This creates a view of all events whose timestamps fall within the provided range.
    /// The range can be specified using Rust's range syntax (`a..b`, `a..=b`, `..b`, etc.).
    ///
    /// Note that the range is interpreted as timestamps, not indices:
    /// - Start bound (inclusive or exclusive) refers to the timestamp, not array position
    /// - End bound (inclusive or exclusive) refers to the timestamp, not array position
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped, TimestampedCollection, TimestampedCollectionMut};
    /// let mut buffer = EventBuffer::new(16);
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(30, "c");
    ///
    /// // Get events with timestamps in range 10..30 (includes 10, excludes 30)
    /// let slice = buffer.slice().slice_by_timestamp(10..30);
    /// assert_eq!(slice.len(), 2); // Contains events with timestamps 10 and 20
    /// ```
    pub fn slice_by_timestamp<R>(&self, range: R) -> &Self
    where
        R: ops::RangeBounds<usize>,
    {
        let range = self.index_range(range);
        self.in_range(range.start, range.end)
    }

    /// Returns a mutable subslice of this [`EventSlice`] based on the provided timestamp range.
    ///
    /// This creates a view of all events whose timestamps fall within the provided range.
    /// The range can be specified using Rust's range syntax (`a..b`, `a..=b`, `..b`, etc.).
    ///
    /// Note that the range is interpreted as timestamps, not indices:
    /// - Start bound (inclusive or exclusive) refers to the timestamp, not array position
    /// - End bound (inclusive or exclusive) refers to the timestamp, not array position
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped, TimestampedCollection, TimestampedCollectionMut};
    /// let mut buffer = EventBuffer::new(16);
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(30, "c");
    ///
    /// // Get events with timestamps in range 10..30 (includes 10, excludes 30)
    /// let slice = buffer.slice_by_timestamp(10..30);
    /// assert_eq!(slice.len(), 2); // Contains events with timestamps 10 and 20
    /// ```
    pub fn slice_mut<R>(&mut self, range: R) -> &mut Self
    where
        R: ops::RangeBounds<usize>,
    {
        let range = self.index_range(range);
        self.in_range_mut(range.start, range.end)
    }

    /// Returns a subslice of this [`EventSlice`] based on the provided index range.
    ///
    /// This creates a view into the underlying events by array indices rather than timestamps.
    /// Use this method when you need to slice by position rather than timestamp.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped, TimestampedCollection, TimestampedCollectionMut};
    /// let mut buffer = EventBuffer::new(16);
    /// buffer.push(100, "a");
    /// buffer.push(200, "b");
    /// buffer.push(300, "c");
    ///
    /// // Get second event by index (regardless of its timestamp)
    /// let slice = buffer.slice_by_index(1..2);
    /// assert_eq!(slice.len(), 1);
    /// assert_eq!(slice.first().unwrap().timestamp, 200);
    /// ```
    pub fn slice_by_index<R>(&self, range: R) -> &Self
    where
        R: slice::SliceIndex<[Timestamped<T>], Output = [Timestamped<T>]>,
    {
        // Safety: The memory layout of `EventSlice<T>` is identical to `[Timestamped<T>]`
        // due to the #[repr(transparent)] attribute
        unsafe { mem::transmute(&self.events[range]) }
    }

    /// Returns a mutable subslice of this [`EventSlice`] based on the provided index range.
    ///
    /// This creates a view into the underlying events by array indices rather than timestamps.
    /// Use this method when you need to slice by position rather than timestamp.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped, TimestampedCollection, TimestampedCollectionMut};
    /// let mut buffer = EventBuffer::new(16);
    /// buffer.push(100, "a");
    /// buffer.push(200, "b");
    /// buffer.push(300, "c");
    ///
    /// // Get second event by index (regardless of its timestamp)
    /// let slice = buffer.slice_by_index(1..2);
    /// assert_eq!(slice.len(), 1);
    /// assert_eq!(slice.first().unwrap().timestamp, 200);
    /// ```
    pub fn slice_by_index_mut<R>(&mut self, range: R) -> &mut Self
    where
        R: slice::SliceIndex<[Timestamped<T>], Output = [Timestamped<T>]>,
    {
        // Safety: The memory layout of `EventSlice<T>` is identical to `[Timestamped<T>]`
        // due to the #[repr(transparent)] attribute
        unsafe { mem::transmute::<&mut [Timestamped<T>], &mut Self>(&mut self.events[range]) }
    }

    /// Returns a reference to the entire underlying slice.
    ///
    /// This provides direct access to the array of [`Timestamped<T>`] events.
    pub fn as_raw_slice(&self) -> &[Timestamped<T>] {
        &self.events
    }

    /// Returns the minimum timestamp in this event slice, if any.
    ///
    /// Returns `None` if the slice is empty. Otherwise, returns the timestamp
    /// of the first event (which has the earliest timestamp).
    pub fn min_timestamp(&self) -> Option<usize> {
        self.first().map(|event| event.timestamp)
    }

    /// Returns the maximum timestamp in this event slice, if any.
    ///
    /// Returns `None` if the slice is empty. Otherwise, returns the timestamp
    /// of the last event (which has the latest timestamp).
    pub fn max_timestamp(&self) -> Option<usize> {
        self.last().map(|event| event.timestamp)
    }

    /// Returns the range of timestamps in this event slice as (min, max),
    /// or `None` if the slice is empty.
    ///
    /// This provides the full time span covered by the events in this slice.
    pub fn timestamp_range(&self) -> Option<(usize, usize)> {
        if self.is_empty() {
            None
        } else {
            Some((self.min_timestamp().unwrap(), self.max_timestamp().unwrap()))
        }
    }

    /// Returns a reference to all events with timestamps less than the specified timestamp.
    ///
    /// This efficiently uses binary search to find all events that occurred before
    /// the given timestamp. The returned slice excludes any event with the exact
    /// timestamp provided.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped, TimestampedCollection, TimestampedCollectionMut};
    /// let mut buffer = EventBuffer::new(16);
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(30, "c");
    ///
    /// let early_events = buffer.slice().before(25);
    /// assert_eq!(early_events.len(), 2); // Contains events with timestamps 10 and 20
    /// ```
    pub fn before(&self, timestamp: usize) -> &Self {
        match self.events.binary_search_by_key(&timestamp, |e| e.timestamp) {
            Ok(idx) | Err(idx) => Self::from_slice(&self.events[..idx]),
        }
    }

    /// Returns a mutable reference to all events with timestamps less than the specified timestamp.
    ///
    /// This efficiently uses binary search to find all events that occurred before
    /// the given timestamp. The returned slice excludes any event with the exact
    /// timestamp provided.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped, TimestampedCollection, TimestampedCollectionMut};
    /// let mut buffer = EventBuffer::new(16);
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(30, "c");
    ///
    /// let early_events = buffer.slice().before(25);
    /// assert_eq!(early_events.len(), 2); // Contains events with timestamps 10 and 20
    /// ```
    pub fn before_mut(&mut self, timestamp: usize) -> &mut Self {
        match self.events.binary_search_by_key(&timestamp, |e| e.timestamp) {
            Ok(idx) | Err(idx) => Self::from_mut_slice(&mut self.events[..idx]),
        }
    }

    /// Returns a reference to all events with timestamps greater than or equal to the specified timestamp.
    ///
    /// This efficiently uses binary search to find all events that occurred at or after
    /// the given timestamp. The returned slice includes any event with the exact
    /// timestamp provided.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped, TimestampedCollection, TimestampedCollectionMut};
    /// let mut buffer = EventBuffer::new(16);
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(30, "c");
    ///
    /// let later_events = buffer.slice().after(20);
    /// assert_eq!(later_events.len(), 2); // Contains events with timestamps 20 and 30
    /// ```
    pub fn after(&self, timestamp: usize) -> &Self {
        match self.events.binary_search_by_key(&timestamp, |e| e.timestamp) {
            Ok(idx) => Self::from_slice(&self.events[idx..]),
            Err(idx) => Self::from_slice(&self.events[idx..]),
        }
    }

    /// Returns a mutable reference to all events with timestamps greater than or equal to the specified timestamp.
    ///
    /// This efficiently uses binary search to find all events that occurred at or after
    /// the given timestamp. The returned slice includes any event with the exact
    /// timestamp provided.
    ///
    /// # Examples
    ///
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped, TimestampedCollection, TimestampedCollectionMut};
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(30, "c");
    ///
    /// let later_events = buffer.slice().after(20);
    /// assert_eq!(later_events.len(), 2); // Contains events with timestamps 20 and 30
    /// ```
    pub fn after_mut(&mut self, timestamp: usize) -> &mut Self {
        match self.events.binary_search_by_key(&timestamp, |e| e.timestamp) {
            Ok(idx) => Self::from_mut_slice(&mut self.events[idx..]),
            Err(idx) => Self::from_mut_slice(&mut self.events[idx..]),
        }
    }

    /// Returns a reference to all events with timestamps in the specified range.
    ///
    /// This returns events with timestamps in the range [start, end).
    /// That is, it includes the start timestamp but excludes the end timestamp.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped, TimestampedCollection, TimestampedCollectionMut};
    /// let mut buffer = EventBuffer::new(16);
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(30, "c");
    /// buffer.push(40, "d");
    ///
    /// let mid_events = buffer.slice().in_range(15, 35);
    /// assert_eq!(mid_events.len(), 2); // Contains events with timestamps 20 and 30
    /// ```
    pub fn in_range(&self, start: usize, end: usize) -> &Self {
        self.after(start).before(end)
    }

    /// Returns a mutable reference to all events with timestamps in the specified range.
    ///
    /// This returns events with timestamps in the range [start, end).
    /// That is, it includes the start timestamp but excludes the end timestamp.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped, TimestampedCollection, TimestampedCollectionMut};
    /// let mut buffer = EventBuffer::new(16);
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(30, "c");
    /// buffer.push(40, "d");
    ///
    /// let mid_events = buffer.slice().in_range(15, 35);
    /// assert_eq!(mid_events.len(), 2); // Contains events with timestamps 20 and 30
    /// ```
    pub fn in_range_mut(&mut self, start: usize, end: usize) -> &mut Self {
        self.after_mut(start).before_mut(end)
    }

    /// Returns the range spanned by events of a particular timestamp, or an empty range to represent no events
    /// happening at that timestamp.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped, TimestampedCollection, TimestampedCollectionMut};
    /// let mut buffer = EventBuffer::new(4);
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(20, "c");
    ///
    /// let range_zero = buffer.range_for_timestamp(0);
    /// assert!(range_zero.is_empty());
    ///
    /// let range_ten = buffer.range_for_timestamp(10);
    /// assert_eq!(0..1, range_ten);
    ///
    /// let range_twenty = buffer.range_for_timestamp(20);
    /// assert_eq!(1..3, range_twenty);
    /// ```
    pub fn range_for_timestamp(&self, timestamp: usize) -> Range<usize> {
        let Some(start) = self.events.iter().position(|e| e.timestamp == timestamp) else {
            return 0..0;
        };
        let mut end = start;
        while end < self.events.len() && self.events[end].timestamp == timestamp {
            end += 1;
        }
        start..end
    }

    /// Returns an iterator that "chunks" an audio block into smaller blocks that can be processed at once, with the
    /// provided events happening at the beginning of each block.
    ///
    /// # Example
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped, TimestampedCollection, TimestampedCollectionMut};
    ///
    /// let mut buffer = EventBuffer::new(4);
    /// buffer.push(1, "a");
    /// buffer.push(3, "b");
    /// buffer.push(3, "c");
    /// buffer.push(6, "d");
    ///
    /// let actual = buffer.slice()
    ///     .chunk_events(16)
    ///     .map(|(range, slice)| (range, slice.into_iter().map(|e| e.data).collect::<Vec<_>>()))
    ///     .collect::<Vec<_>>();
    /// let expected = vec![
    ///     (0..1, vec![]),
    ///     (1..3, vec!["a"]),
    ///     (3..6, vec!["b", "c"]),
    ///     (6..16, vec!["d"]),
    /// ];
    /// assert_eq!(expected, actual);
    pub fn chunk_events(&self, block_size: usize) -> impl Iterator<Item = (Range<usize>, &Self)> {
        let mut start = 0;
        std::iter::from_fn(move || {
            if start >= block_size {
                return None;
            }

            let end = self
                .events
                .iter()
                .find(|e| e.timestamp > start)
                .map(|e| e.timestamp)
                .unwrap_or(block_size);
            let index_range = self.timestamp_index_range(start..end);
            let timestamp_range = start..end;
            start = end;
            Some((timestamp_range, &self[index_range]))
        })
    }

    fn timestamp_index_range<R>(&self, range: R) -> Range<usize>
    where
        R: ops::RangeBounds<usize>,
    {
        let find_bound = |t| {
            self.events
                .iter()
                .position(|e| e.timestamp >= t)
                .unwrap_or(self.events.len())
        };
        let start_bound = match range.start_bound() {
            ops::Bound::Included(&t) => find_bound(t),
            ops::Bound::Excluded(&t) => find_bound(t + 1),
            ops::Bound::Unbounded => 0,
        };

        let end_bound = match range.end_bound() {
            ops::Bound::Included(&t) => find_bound(t + 1),
            ops::Bound::Excluded(&t) => find_bound(t),
            ops::Bound::Unbounded => self.len(),
        };
        start_bound..end_bound
    }

    fn index_range<R>(&self, range: R) -> Range<usize>
    where
        R: ops::RangeBounds<usize>,
    {
        let start_bound = match range.start_bound() {
            ops::Bound::Included(&t) => t,
            ops::Bound::Excluded(&t) => t + 1, // Convert exclusive to inclusive
            ops::Bound::Unbounded => 0,
        };

        let end_bound = match range.end_bound() {
            ops::Bound::Included(&t) => t + 1, // Convert inclusive to exclusive
            ops::Bound::Excluded(&t) => t,
            ops::Bound::Unbounded => self.len(),
        };

        start_bound..end_bound
    }
}

impl<T> ops::Deref for EventSlice<T> {
    type Target = [Timestamped<T>];

    fn deref(&self) -> &Self::Target {
        &self.events
    }
}

impl<T> ops::DerefMut for EventSlice<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.events
    }
}

impl<T> TimestampedCollection<T> for EventSlice<T> {
    fn slice(&self) -> &EventSlice<T> {
        self
    }
}

impl<T> ops::Index<usize> for EventSlice<T> {
    type Output = Timestamped<T>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.events[index]
    }
}

impl<T: Copy> ops::IndexMut<usize> for EventSlice<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.events[index]
    }
}

macro_rules! impl_index {
    ($ty:ty) => {
        impl<T> ops::Index<$ty> for EventSlice<T> {
            type Output = Self;

            fn index(&self, range: $ty) -> &Self::Output {
                self.slice_by_index(range)
            }
        }

        impl<T> ops::IndexMut<$ty> for EventSlice<T> {
            fn index_mut(&mut self, range: $ty) -> &mut Self::Output {
                self.slice_by_index_mut(range)
            }
        }
    };

    (@ByTimestamp $ty:ty) => {
        impl<T> ops::Index<ByTimestamp<$ty>> for EventSlice<T> {
            type Output = Self;

            fn index(&self, range: ByTimestamp<$ty>) -> &Self::Output {
                let range = self.timestamp_index_range(range.0);
                self.slice_by_index(range)
            }
        }

        impl<T> ops::IndexMut<ByTimestamp<$ty>> for EventSlice<T> {
            fn index_mut(&mut self, range: ByTimestamp<$ty>) -> &mut Self::Output {
                let range = self.timestamp_index_range(range.0);
                self.slice_by_index_mut(range)
            }
        }
    };
}

impl_index!(Range<usize>);
impl_index!(RangeFrom<usize>);
impl_index!(RangeTo<usize>);
impl_index!(RangeFull);
impl_index!(RangeInclusive<usize>);
impl_index!(RangeToInclusive<usize>);

/// A wrapper struct to indicate timestamp-based slicing rather than index-based slicing.
///
/// This helps distinguish between the two types of slicing operations when using the
/// indexing operator (`[]`). Wrap your range or index in [`ByTimestamp`] to slice based on
/// timestamp rather than index.
///
/// # Examples
///
/// ```
/// # use clogbox_module::eventbuffer::{EventBuffer, ByTimestamp, TimestampedCollection, TimestampedCollectionMut};
/// let mut buffer = EventBuffer::new(4);
/// buffer.push(100, "a");
/// buffer.push(200, "b");
/// buffer.push(300, "c");
///
/// // By default, indexing uses timestamps
/// let slice1 = &buffer.slice()[ByTimestamp(100..250)]; // Events with timestamps between 100-250
///
/// // Use ByIndex to index by array position
/// let slice2 = &buffer.slice()[0..2]; // First 2 events regardless of timestamp
///
/// assert!(std::ptr::eq(slice1, slice2));
/// ```
pub struct ByTimestamp<R>(pub R);

impl<T> ops::Index<ByTimestamp<usize>> for EventSlice<T> {
    type Output = Timestamped<T>;

    fn index(&self, index: ByTimestamp<usize>) -> &Self::Output {
        &self.events[index.0]
    }
}

impl_index!(@ByTimestamp Range<usize>);
impl_index!(@ByTimestamp RangeFrom<usize>);
impl_index!(@ByTimestamp RangeTo<usize>);
impl_index!(@ByTimestamp RangeInclusive<usize>);
impl_index!(@ByTimestamp RangeToInclusive<usize>);
impl_index!(@ByTimestamp RangeFull);

// No longer needed, as we're now using the from_slice static method

/// A buffer that maintains timestamped events in sorted order by timestamp.
///
/// [`EventBuffer`] automatically keeps events sorted by their timestamps, enabling
/// efficient time-based filtering and querying. It provides methods for adding,
/// removing, and accessing events based on their timestamps or array indices.
///
/// # Examples
///
/// ```
/// # use clogbox_module::eventbuffer::{EventBuffer, TimestampedCollection, TimestampedCollectionMut};
/// // Create a buffer of timestamped strings
/// let mut buffer = EventBuffer::new(16);
///
/// // Add events with timestamps (automatically sorted)
/// buffer.push(100, "event at t=100");
/// buffer.push(50, "event at t=50");   // Will be inserted at the beginning
/// buffer.push(200, "event at t=200");
///
/// // Access events
/// assert_eq!(buffer.len(), 3);
/// assert_eq!(buffer.first().unwrap().data, "event at t=50");
///
/// // Filter events by time
/// let recent_events = buffer.after(100);
/// assert_eq!(recent_events.len(), 2); // Events at t=100 and t=200
/// ```
///
/// # Invariants
///
/// - `self.events[..self.len]` contains initialized elements
/// - `self.events[self..]` contains uninitialized elements
#[derive(Debug)]
pub struct EventBuffer<T> {
    events: Box<[MaybeUninit<Timestamped<T>>]>,
    len: usize,
}

impl<T> ops::Deref for EventBuffer<T> {
    type Target = EventSlice<T>;

    fn deref(&self) -> &Self::Target {
        self.slice()
    }
}

impl<T> ops::DerefMut for EventBuffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.slice_mut()
    }
}

impl<T> TimestampedCollection<T> for EventBuffer<T> {
    fn slice(&self) -> &EventSlice<T> {
        EventSlice::from_slice(unsafe {
            mem::transmute::<&[MaybeUninit<Timestamped<T>>], &[Timestamped<T>]>(&self.events[..self.len])
        })
    }
}

impl<T> TimestampedCollectionMut<T> for EventBuffer<T> {
    fn slice_mut(&mut self) -> &mut EventSlice<T> {
        EventSlice::from_mut_slice(unsafe {
            mem::transmute::<&mut [MaybeUninit<Timestamped<T>>], &mut [Timestamped<T>]>(&mut self.events[..self.len])
        })
    }

    fn push(&mut self, timestamp: usize, data: T) -> bool {
        if self.len == self.capacity() {
            return false;
        }
        let event = Timestamped { timestamp, data };

        // Find the insertion position using binary search
        match self.slice().events.binary_search_by_key(&timestamp, |e| e.timestamp) {
            Ok(idx) => {
                // If we found an exact match, insert after that position
                self.insert(idx + 1, event);
            }
            Err(idx) => {
                // Otherwise, insert at the position where it should be
                self.insert(idx, event);
            }
        }
        true
    }
}

impl<T> EventBuffer<T> {
    /// Creates a new, empty [`EventBuffer`].
    ///
    /// The buffer is initially created with no allocated memory.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::EventBuffer;
    /// let buffer: EventBuffer<&str> = EventBuffer::new(16);
    /// assert!(buffer.is_empty());
    /// ```
    pub fn new(capacity: usize) -> Self {
        Self {
            events: Box::from_iter(std::iter::repeat_with(|| MaybeUninit::uninit()).take(capacity)),
            len: 0,
        }
    }

    /// Returns the number of events currently stored in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the event buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the capacity of the event buffer
    pub fn capacity(&self) -> usize {
        self.events.len()
    }

    /// Clears the events contained in this buffer. This effectively sets the length to zero while keeping the
    /// allocation intact.
    pub fn clear(&mut self) {
        if mem::needs_drop::<T>() {
            unsafe {
                for event in self.events.iter_mut().take(self.len) {
                    ptr::drop_in_place(event.as_mut_ptr());
                }
            }
        }
        self.len = 0;
    }

    /// Removes all events with timestamps less than the specified timestamp.
    pub fn trim_before(&mut self, timestamp: usize) {
        let idx = self
            .events
            .binary_search_by_key(&timestamp, |e| unsafe { e.assume_init_ref() }.timestamp)
            .unwrap_or_else(|idx| idx);
        self.len -= idx;
        if mem::needs_drop::<T>() {
            unsafe {
                for event in self.events.iter_mut().take(idx) {
                    ptr::drop_in_place(event.as_mut_ptr());
                }
            }
        }
        unsafe {
            ptr::copy(&raw mut self.events[idx], &raw mut self.events[0], self.len);
        }
    }

    /// Removes all events with timestamps greater than the specified timestamp.
    pub fn trim_after(&mut self, timestamp: usize) {
        let idx = self
            .events
            .binary_search_by_key(&timestamp, |e| unsafe { e.assume_init_ref() }.timestamp);
        let idx = idx.map(|idx| idx + 1).unwrap_or_else(|idx| idx);
        if mem::needs_drop::<T>() {
            unsafe {
                for event in self.events.iter_mut().skip(idx) {
                    ptr::drop_in_place(event.as_mut_ptr());
                }
            }
        }
        self.len = idx;
    }

    /// Returns a reference to an `EventSlice` containing all events with timestamps less than the specified timestamp.
    pub fn before(&self, timestamp: usize) -> &EventSlice<T> {
        self.slice().before(timestamp)
    }

    /// Returns a reference to an `EventSlice` containing all events with timestamps greater than or equal to the specified timestamp.
    pub fn after(&self, timestamp: usize) -> &EventSlice<T> {
        self.slice().after(timestamp)
    }

    /// Returns a reference to an `EventSlice` containing all events with timestamps in the specified range.
    pub fn in_range(&self, start: usize, end: usize) -> &EventSlice<T> {
        self.slice().in_range(start, end)
    }

    /// Returns a reference to an `EventSlice` based on the provided index range.
    ///
    /// This creates a view into the underlying events by raw indices rather than timestamps.
    pub fn slice_by_index<R>(&self, range: R) -> &EventSlice<T>
    where
        R: slice::SliceIndex<[Timestamped<T>], Output = [Timestamped<T>]>,
    {
        self.slice().slice_by_index(range)
    }

    /// Returns a mutable reference to an `EventSlice` based on the provided index range.
    ///
    /// This creates a view into the underlying events by raw indices rather than timestamps.
    pub fn slice_by_index_mut<R>(&mut self, range: R) -> &mut EventSlice<T>
    where
        R: slice::SliceIndex<[Timestamped<T>], Output = [Timestamped<T>]>,
    {
        self.slice_mut().slice_by_index_mut(range)
    }

    fn insert(&mut self, index: usize, value: Timestamped<T>) {
        debug_assert!(index <= self.len);
        if index == self.len {
            self.events[index] = MaybeUninit::new(value);
            self.len += 1;
            return;
        }
        let to_move = self.len - index;
        unsafe {
            ptr::copy(&self.events[index], &raw mut self.events[index + 1], to_move);
        }
        self.events[index] = MaybeUninit::new(value);
        self.len += 1;
    }
}

/// Iterator moving elements out of [`EventBuffer`].
pub struct IntoIter<T> {
    events: Box<[MaybeUninit<Timestamped<T>>]>,
    len: usize,
    index: usize,
}

impl<T> Iterator for IntoIter<T> {
    type Item = Timestamped<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let event = mem::replace(&mut self.events[self.index], MaybeUninit::uninit());
        self.index += 1;
        Some(unsafe { event.assume_init() })
    }
}

impl<T> IntoIterator for EventBuffer<T> {
    type Item = Timestamped<T>;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            events: self.events,
            len: self.len,
            index: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_maintains_order() {
        let mut buffer = EventBuffer::new(4);

        buffer.push(3, "c");
        buffer.push(1, "a");
        buffer.push(2, "b");
        buffer.push(4, "d");

        let result: Vec<_> = buffer.iter().map(|e| (e.timestamp, e.data)).collect();
        assert_eq!(result, vec![(1, "a"), (2, "b"), (3, "c"), (4, "d")]);
    }

    #[test]
    fn test_push_with_duplicate_timestamps() {
        let mut buffer = EventBuffer::new(4);

        buffer.push(1, "a");
        buffer.push(2, "b1");
        buffer.push(2, "b2");
        buffer.push(3, "c");

        let result: Vec<_> = buffer.iter().map(|e| (e.timestamp, e.data)).collect();
        assert_eq!(result, vec![(1, "a"), (2, "b1"), (2, "b2"), (3, "c")]);
    }

    #[test]
    fn test_trim_operations() {
        let mut buffer = EventBuffer::new(5);

        buffer.push(1, "a");
        buffer.push(2, "b");
        buffer.push(3, "c");
        buffer.push(4, "d");
        buffer.push(5, "e");

        buffer.trim_before(3);
        let result1: Vec<_> = buffer.iter().map(|e| (e.timestamp, e.data)).collect();
        assert_eq!(result1, vec![(3, "c"), (4, "d"), (5, "e")]);

        buffer.trim_after(4);
        let result2: Vec<_> = buffer.iter().map(|e| (e.timestamp, e.data)).collect();
        assert_eq!(result2, vec![(3, "c"), (4, "d")]);
    }

    #[test]
    fn test_iter_mut_resorts() {
        let mut buffer = EventBuffer::new(4);

        buffer.push(1, "a");
        buffer.push(3, "c");
        buffer.push(5, "e");

        // Modify timestamps during iteration
        {
            let iter = buffer.iter_mut();
            for event in iter {
                // Reverse the timestamps
                event.timestamp = 6 - event.timestamp;
            }
            // Buffer will be re-sorted when iter goes out of scope
        }

        // Check that the buffer was re-sorted
        let result: Vec<_> = buffer.iter().map(|e| (e.timestamp, e.data)).collect();
        assert_eq!(result, vec![(1, "e"), (3, "c"), (5, "a")]);
    }

    #[test]
    fn test_event_slice() {
        let mut buffer = EventBuffer::new(5);

        buffer.push(1, "a");
        buffer.push(2, "b");
        buffer.push(3, "c");
        buffer.push(4, "d");
        buffer.push(5, "e");

        // Test as_slice and basic operations
        let event_slice = buffer.slice();
        assert_eq!(event_slice.len(), 5);
        assert_eq!(*event_slice.get(2).unwrap().data, "c");
        assert_eq!(event_slice.first().unwrap().data, "a");
    }

    #[test]
    fn test_event_slice_chunk_empty() {
        let buffer = EventBuffer::<()>::new(5);
        let data = buffer.chunk_events(16).collect::<Vec<_>>();
        assert_eq!(1, data.len());
        assert_eq!(0..16, data[0].0);
    }

    #[test]
    fn test_event_slice_chunk() {
        let mut buffer = EventBuffer::new(4);
        buffer.push(1, "a");
        buffer.push(3, "b");
        buffer.push(3, "c");
        buffer.push(6, "d");

        let actual = buffer
            .slice()
            .chunk_events(16)
            .map(|(range, slice)| (range, slice.into_iter().map(|e| e.data).collect::<Vec<_>>()))
            .collect::<Vec<_>>();
        let expected = vec![
            (0..1, vec![]),
            (1..3, vec!["a"]),
            (3..6, vec!["b", "c"]),
            (6..16, vec!["d"]),
        ];
        assert_eq!(expected, actual);
    }
}
