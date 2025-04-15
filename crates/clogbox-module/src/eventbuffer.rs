use std::{ops, slice};

/// A wrapper for data with an associated timestamp.
///
/// Provides a container that pairs data with a numeric timestamp for chronological ordering.
#[derive(Debug, Clone)]
pub struct Timestamped<T> {
    /// The timestamp of the event.
    pub timestamp: usize,
    /// The event data.
    pub data: T,
}

impl<T> ops::Deref for Timestamped<T> {
    type Target = T;

    /// Returns a reference to the inner data.
    ///
    /// This allows using a [`Timestamped<T>`] value anywhere a `&T` is expected.
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> ops::DerefMut for Timestamped<T> {
    /// Returns a mutable reference to the inner data.
    ///
    /// This allows using a [`Timestamped<T>`] value anywhere a `&mut T` is expected.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<T> PartialEq for Timestamped<T> {
    /// Compares [`Timestamped`] values based on their timestamps only.
    ///
    /// Note: This ignores the inner data and only compares timestamps.
    fn eq(&self, other: &Self) -> bool {
        self.timestamp.eq(&other.timestamp)
    }
}

impl<T> Eq for Timestamped<T> {}

impl<T> PartialOrd for Timestamped<T> {
    /// Compares [`Timestamped`] values based on their timestamps.
    ///
    /// Always returns `Some` since timestamps are comparable.
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Timestamped<T> {
    /// Orders [`Timestamped`] values based on their timestamps.
    ///
    /// This enables chronological sorting of events.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.timestamp.cmp(&other.timestamp)
    }
}

/// A mutable iterator that resorts the buffer when dropped.
///
/// This iterator allows modifying [`Timestamped`] entries in an [`EventBuffer`].
/// When the iterator goes out of scope (is dropped), the underlying buffer is
/// automatically re-sorted by timestamp to maintain chronological order.
pub struct IterMut<'a, T> {
    events: &'a mut Vec<Timestamped<T>>,
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
        // This is safe because we ensure the lifetime is tied to the IterMut lifetime
        // and each item is yielded only once.
        let events = unsafe { std::mem::transmute::<&mut Vec<Timestamped<T>>, &mut Vec<Timestamped<T>>>(self.events) };
        
        if self.index < events.len() {
            let item = &mut events[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

impl<'a, T> Drop for IterMut<'a, T> {
    /// Automatically re-sorts the underlying buffer when the iterator is dropped.
    ///
    /// This ensures chronological ordering after timestamp modifications.
    fn drop(&mut self) {
        // Re-sort the events when the iterator is dropped
        self.events.sort();
    }
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
        // Safety: The memory layout of `EventSlice<T>` is identical to `[Timestamped<T>]`
        // due to the #[repr(transparent)] attribute
        unsafe { std::mem::transmute(events) }
    }

    /// Returns the number of events in the slice.
    ///
    /// This is equivalent to the [`len`](slice::len) method on slices.
    pub fn len(&self) -> usize {
        self.events.len()
    }
    
    /// Returns `true` if the slice contains no events.
    ///
    /// This is equivalent to the [`is_empty`](slice::is_empty) method on slices.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
    
    /// Returns a reference to the event at the specified index, if it exists.
    ///
    /// Returns `None` if the index is out of bounds.
    pub fn get(&self, index: usize) -> Option<&Timestamped<T>> {
        self.events.get(index)
    }
    
    /// Returns a reference to the first event, if it exists.
    ///
    /// The first event is the one with the earliest timestamp, since
    /// events are maintained in chronological order.
    pub fn first(&self) -> Option<&Timestamped<T>> {
        self.events.first()
    }
    
    /// Returns a reference to the last event, if it exists.
    ///
    /// The last event is the one with the latest timestamp, since
    /// events are maintained in chronological order.
    pub fn last(&self) -> Option<&Timestamped<T>> {
        self.events.last()
    }
    
    /// Returns an iterator over the events.
    ///
    /// Events are yielded in chronological order (by timestamp).
    pub fn iter(&self) -> impl Iterator<Item = &Timestamped<T>> {
        self.events.iter()
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
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped};
    /// let mut buffer = EventBuffer::new();
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(30, "c");
    ///
    /// // Get events with timestamps in range 10..30 (includes 10, excludes 30)
    /// let slice = buffer.as_slice().slice(10..30);
    /// assert_eq!(slice.len(), 2); // Contains events with timestamps 10 and 20
    /// ```
    pub fn slice<R>(&self, range: R) -> &Self
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
            ops::Bound::Unbounded => usize::MAX,
        };
        
        self.in_range(start_bound, end_bound)
    }
    
    /// Returns a subslice of this [`EventSlice`] based on the provided index range.
    ///
    /// This creates a view into the underlying events by array indices rather than timestamps.
    /// Use this method when you need to slice by position rather than timestamp.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped};
    /// let mut buffer = EventBuffer::new();
    /// buffer.push(100, "a");
    /// buffer.push(200, "b");
    /// buffer.push(300, "c");
    ///
    /// // Get second event by index (regardless of its timestamp)
    /// let slice = buffer.as_slice().slice_by_index(1..2);
    /// assert_eq!(slice.len(), 1);
    /// assert_eq!(slice.first().unwrap().timestamp, 200);
    /// ```
    pub fn slice_by_index<R>(&self, range: R) -> &Self
    where
        R: std::slice::SliceIndex<[Timestamped<T>], Output = [Timestamped<T>]>,
    {
        // Safety: The memory layout of `EventSlice<T>` is identical to `[Timestamped<T>]`
        // due to the #[repr(transparent)] attribute
        unsafe { std::mem::transmute(&self.events[range]) }
    }

    /// Returns a reference to the entire underlying slice.
    ///
    /// This provides direct access to the array of [`Timestamped<T>`] events.
    pub fn as_slice(&self) -> &[Timestamped<T>] {
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
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped};
    /// let mut buffer = EventBuffer::new();
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(30, "c");
    ///
    /// let early_events = buffer.as_slice().before(25);
    /// assert_eq!(early_events.len(), 2); // Contains events with timestamps 10 and 20
    /// ```
    pub fn before(&self, timestamp: usize) -> &Self {
        match self.events.binary_search_by_key(&timestamp, |e| e.timestamp) {
            Ok(idx) | Err(idx) => Self::from_slice(&self.events[..idx]),
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
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped};
    /// let mut buffer = EventBuffer::new();
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(30, "c");
    ///
    /// let later_events = buffer.as_slice().after(20);
    /// assert_eq!(later_events.len(), 2); // Contains events with timestamps 20 and 30
    /// ```
    pub fn after(&self, timestamp: usize) -> &Self {
        match self.events.binary_search_by_key(&timestamp, |e| e.timestamp) {
            Ok(idx) => Self::from_slice(&self.events[idx..]),
            Err(idx) => Self::from_slice(&self.events[idx..]),
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
    /// # use clogbox_module::eventbuffer::{EventBuffer, Timestamped};
    /// let mut buffer = EventBuffer::new();
    /// buffer.push(10, "a");
    /// buffer.push(20, "b");
    /// buffer.push(30, "c");
    /// buffer.push(40, "d");
    ///
    /// let mid_events = buffer.as_slice().in_range(15, 35);
    /// assert_eq!(mid_events.len(), 2); // Contains events with timestamps 20 and 30
    /// ```
    pub fn in_range(&self, start: usize, end: usize) -> &Self {
        self.after(start).before(end)
    }
}

impl<T> ops::Index<usize> for EventSlice<T> {
    type Output = Timestamped<T>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.events[index]
    }
}

impl<T> ops::Index<ops::Range<usize>> for EventSlice<T> {
    type Output = [Timestamped<T>];

    fn index(&self, range: ops::Range<usize>) -> &Self::Output {
        // By default, use timestamp-based indexing
        self.slice(range).as_slice()
    }
}

impl<T> ops::Index<ops::RangeTo<usize>> for EventSlice<T> {
    type Output = [Timestamped<T>];

    fn index(&self, range: ops::RangeTo<usize>) -> &Self::Output {
        // By default, use timestamp-based indexing
        self.slice(range).as_slice()
    }
}

impl<T> ops::Index<ops::RangeFrom<usize>> for EventSlice<T> {
    type Output = [Timestamped<T>];

    fn index(&self, range: ops::RangeFrom<usize>) -> &Self::Output {
        // By default, use timestamp-based indexing
        self.slice(range).as_slice()
    }
}

impl<T> ops::Index<ops::RangeFull> for EventSlice<T> {
    type Output = [Timestamped<T>];

    fn index(&self, range: ops::RangeFull) -> &Self::Output {
        &self.events[range]
    }
}

/// A wrapper struct to indicate index-based slicing rather than timestamp-based slicing.
///
/// This helps distinguish between the two types of slicing operations when using the
/// indexing operator (`[]`). Wrap your range or index in [`ByIndex`] to slice based on
/// array position rather than timestamp.
///
/// # Examples
///
/// ```
/// # use clogbox_module::eventbuffer::{EventBuffer, ByIndex};
/// let mut buffer = EventBuffer::new();
/// buffer.push(100, "a");
/// buffer.push(200, "b");
/// buffer.push(300, "c");
///
/// // By default, indexing uses timestamps
/// let slice1 = &buffer.as_slice()[100..250]; // Events with timestamps between 100-250
///
/// // Use ByIndex to index by array position
/// let slice2 = &buffer.as_slice()[ByIndex(0..2)]; // First 2 events regardless of timestamp
/// ```
pub struct ByIndex<R>(pub R);

impl<T, R: slice::SliceIndex<[Timestamped<T>]>> ops::Index<ByIndex<R>> for EventSlice<T> {
    type Output = R::Output;

    fn index(&self, index: ByIndex<R>) -> &Self::Output {
        &self.events[index.0]
    }
}

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
/// # use clogbox_module::eventbuffer::EventBuffer;
/// // Create a buffer of timestamped strings
/// let mut buffer = EventBuffer::new();
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
#[derive(Debug, Clone)]
pub struct EventBuffer<T> {
    events: Vec<Timestamped<T>>,
}

impl<T> Default for EventBuffer<T> {
    fn default() -> Self {
        Self::new()
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
    /// let buffer: EventBuffer<&str> = EventBuffer::new();
    /// assert!(buffer.is_empty());
    /// ```
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }
    
    /// Returns a reference to an [`EventSlice`] that provides a view into the entire buffer.
    ///
    /// This slice can be used to efficiently query and filter events without copying them.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::eventbuffer::EventBuffer;
    /// let mut buffer = EventBuffer::new();
    /// buffer.push(1, "a");
    /// buffer.push(2, "b");
    ///
    /// let slice = buffer.as_slice();
    /// assert_eq!(slice.len(), 2);
    /// ```
    pub fn as_slice(&self) -> &EventSlice<T> {
        EventSlice::from_slice(&self.events)
    }
    
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
    
    pub fn len(&self) -> usize {
        self.events.len()
    }
    
    pub fn capacity(&self) -> usize {
        self.events.capacity()
    }
    
    /// Creates a new [`EventBuffer`] with the specified capacity.
    ///

    /// Adds an event with the specified timestamp to the buffer,
    /// maintaining sorted order by timestamp.
    pub fn push(&mut self, timestamp: usize, data: T) {
        let event = Timestamped { timestamp, data };
        
        // Find the insertion position using binary search
        match self.events.binary_search_by_key(&timestamp, |e| e.timestamp) {
            Ok(idx) => {
                // If we found an exact match, insert after that position
                self.events.insert(idx + 1, event);
            }
            Err(idx) => {
                // Otherwise, insert at the position where it should be
                self.events.insert(idx, event);
            }
        }
    }

    /// Returns a reference to the event at the specified index, if it exists.
    pub fn get(&self, index: usize) -> Option<&Timestamped<T>> {
        self.events.get(index)
    }

    /// Returns a reference to the first event in the buffer, if it exists.
    pub fn first(&self) -> Option<&Timestamped<T>> {
        self.events.first()
    }

    /// Returns a reference to the last event in the buffer, if it exists.
    pub fn last(&self) -> Option<&Timestamped<T>> {
        self.events.last()
    }

    /// Returns an iterator over the events in the buffer.
    pub fn iter(&self) -> impl Iterator<Item = &Timestamped<T>> {
        self.events.iter()
    }
    
    /// Returns a mutable iterator over the events in the buffer.
    /// The buffer will be automatically re-sorted when the iterator is dropped,
    /// to account for any timestamp changes made during iteration.
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            events: &mut self.events,
            index: 0,
        }
    }
    
    /// Removes all events with timestamps less than the specified timestamp.
    pub fn trim_before(&mut self, timestamp: usize) {
        let idx = self.events.binary_search_by_key(&timestamp, |e| e.timestamp)
            .unwrap_or_else(|idx| idx);
        self.events.drain(0..idx);
    }
    
    /// Removes all events with timestamps greater than the specified timestamp.
    pub fn trim_after(&mut self, timestamp: usize) {
        let idx = self.events.binary_search_by_key(&timestamp, |e| e.timestamp);
        let idx = idx.map(|idx| idx + 1).unwrap_or_else(|idx| idx);
        self.events.truncate(idx);
    }
    
    /// Returns a reference to an `EventSlice` containing all events with timestamps less than the specified timestamp.
    pub fn before(&self, timestamp: usize) -> &EventSlice<T> {
        self.as_slice().before(timestamp)
    }
    
    /// Returns a reference to an `EventSlice` containing all events with timestamps greater than or equal to the specified timestamp.
    pub fn after(&self, timestamp: usize) -> &EventSlice<T> {
        self.as_slice().after(timestamp)
    }
    
    /// Returns a reference to an `EventSlice` containing all events with timestamps in the specified range.
    pub fn in_range(&self, start: usize, end: usize) -> &EventSlice<T> {
        self.as_slice().in_range(start, end)
    }
    
    /// Returns a reference to an `EventSlice` based on the provided timestamp range.
    ///
    /// This creates a view of all events whose timestamps fall within the provided range.
    /// Note that the range is inclusive of the start and exclusive of the end.
    pub fn slice<R>(&self, range: R) -> &EventSlice<T>
    where
        R: ops::RangeBounds<usize>,
    {
        self.as_slice().slice(range)
    }
    
    /// Returns a reference to an `EventSlice` based on the provided index range.
    ///
    /// This creates a view into the underlying events by raw indices rather than timestamps.
    pub fn slice_by_index<R>(&self, range: R) -> &EventSlice<T>
    where
        R: slice::SliceIndex<[Timestamped<T>], Output = [Timestamped<T>]>,
    {
        EventSlice::from_slice(&self.events[range])
    }
}

impl<T> FromIterator<(usize, T)> for EventBuffer<T> {
    fn from_iter<I: IntoIterator<Item = (usize, T)>>(iter: I) -> Self {
        let mut buffer = Self::new();
        for (timestamp, data) in iter {
            buffer.push(timestamp, data);
        }
        buffer
    }
}

impl<T> IntoIterator for EventBuffer<T> {
    type Item = Timestamped<T>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_maintains_order() {
        let mut buffer = EventBuffer::new();
        
        buffer.push(3, "c");
        buffer.push(1, "a");
        buffer.push(2, "b");
        buffer.push(4, "d");
        
        let result: Vec<_> = buffer.iter().map(|e| (e.timestamp, e.data)).collect();
        assert_eq!(result, vec![(1, "a"), (2, "b"), (3, "c"), (4, "d")]);
    }

    #[test]
    fn test_push_with_duplicate_timestamps() {
        let mut buffer = EventBuffer::new();
        
        buffer.push(1, "a");
        buffer.push(2, "b1");
        buffer.push(2, "b2");
        buffer.push(3, "c");
        
        let result: Vec<_> = buffer.iter().map(|e| (e.timestamp, e.data)).collect();
        assert_eq!(result, vec![(1, "a"), (2, "b1"), (2, "b2"), (3, "c")]);
    }

    #[test]
    fn test_trim_operations() {
        let mut buffer = EventBuffer::new();
        
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
        let mut buffer = EventBuffer::new();
        
        buffer.push(1, "a");
        buffer.push(3, "c");
        buffer.push(5, "e");
        
        // Modify timestamps during iteration
        {
            let mut iter = buffer.iter_mut();
            while let Some(event) = iter.next() {
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
            let mut buffer = EventBuffer::new();
            
            buffer.push(1, "a");
            buffer.push(2, "b");
            buffer.push(3, "c");
            buffer.push(4, "d");
            buffer.push(5, "e");
            
            // Test as_slice and basic operations
            let event_slice = buffer.as_slice();
            assert_eq!(event_slice.len(), 5);
            assert_eq!(event_slice.get(2).unwrap().data, "c");
            assert_eq!(event_slice.first().unwrap().data, "a");
    }
}