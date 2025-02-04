use crate::NoteBuffer;
use crate::ParamBuffer;
use derive_more::{Deref, DerefMut};
use std::cell::UnsafeCell;
use std::fmt;
use std::fmt::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A thread-safe, atomic bitset for efficiently managing and manipulating bits.
///
/// This is not a growable type ; once initialized, it has a fixed capacity.
///
/// This type shares data between its clones.
#[derive(Clone)]
pub(crate) struct AtomicBitset {
    bits: Arc<[AtomicU64]>,
}

impl fmt::Debug for AtomicBitset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..self.bits.len() {
            if i > 0 {
                f.write_char('|')?;
            }
            for b in 0..64 {
                let bit = self.bits[i].load(Ordering::SeqCst);
                let bit_mask = 1 << b;
                let bit_set = (bit & bit_mask) != 0;
                f.write_char(if bit_set { 'X' } else { '-' })?;
            }
        }
        Ok(())
    }
}

impl AtomicBitset {
    const ORDERING: Ordering = Ordering::SeqCst;

    pub fn new(capacity: usize) -> Self {
        let len = (capacity + 32) / 64;
        Self {
            bits: std::iter::repeat_with(|| AtomicU64::new(0)).take(len).collect(),
        }
    }

    pub fn get(&self, index: usize) -> bool {
        let bit_index = index / 64;
        let bit_offset = index % 64;
        let bit_mask = 1 << bit_offset;
        let bit = self.bits[bit_index].load(Self::ORDERING);
        (bit & bit_mask) != 0
    }

    pub fn set(&self, index: usize) {
        let bit_index = index / 64;
        let bit_offset = index % 64;
        let bit_mask = 1 << bit_offset;
        self.bits[bit_index].fetch_or(bit_mask, Self::ORDERING);
    }

    pub fn clear(&self, index: usize) {
        let bit_index = index / 64;
        let bit_offset = index % 64;
        let bit_mask = 1 << bit_offset;
        self.bits[bit_index].fetch_and(!bit_mask, Self::ORDERING);
    }
}

/// Represents a borrowed reference to storage, along with metadata for managing borrowing.
///
/// [`SRef`] is a wrapper around a data reference, which is used in conjunction with
/// an [`AtomicBitset`] to manage and track borrowing state.
///
/// # Generic Parameters
///
/// - `T`: The type of the borrowed data (e.g. `&[f32]`).
#[derive(Debug, Deref)]
pub struct SRef<'a, T: ?Sized> {
    #[deref]
    pub data: &'a T,
    pub(crate) index: usize,
    pub(crate) borrow_marker: AtomicBitset,
}

/// Represents a borrowed reference to storage, along with metadata for managing borrowing.
///
/// [`SRef`] is a wrapper around a data reference, which is used in conjunction with
/// an [`AtomicBitset`] to manage and track borrowing state.
///
/// # Generic Parameters
///
/// - `T`: The type of the borrowed data (e.g. `&[f32]`).
#[derive(Debug, Deref, DerefMut)]
pub struct SMut<'a, T: ?Sized> {
    #[deref]
    #[deref_mut]
    pub data: &'a mut T,
    pub(crate) index: usize,
    pub(crate) borrow_marker: AtomicBitset,
}

pub trait SharedStorage {
    type Value: ?Sized;
    fn len(&self) -> usize;
    fn get(&self, index: usize) -> SRef<Self::Value>;
    fn get_mut(&self, index: usize) -> SMut<Self::Value>;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<'a, S: ?Sized + SharedStorage> SharedStorage for &'a S {
    type Value = S::Value;

    fn len(&self) -> usize {
        (**self).len()
    }

    fn get(&self, index: usize) -> SRef<Self::Value> {
        (**self).get(index)
    }

    fn get_mut(&self, index: usize) -> SMut<Self::Value> {
        (**self).get_mut(index)
    }
}

pub struct Storage<T: ?Sized> {
    borrow_marker: AtomicBitset,
    borrow_marker_mut: AtomicBitset,
    buffers: Box<[UnsafeCell<Box<T>>]>,
}

impl<T> FromIterator<T> for Storage<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from_iter(iter.into_iter().map(Box::new))
    }
}

impl<T: ?Sized> FromIterator<Box<T>> for Storage<T> {
    fn from_iter<I: IntoIterator<Item = Box<T>>>(iter: I) -> Self {
        let buffers = iter.into_iter().map(UnsafeCell::new).collect::<Box<[_]>>();
        let borrow_marker = AtomicBitset::new(buffers.len());
        let borrow_marker_mut = AtomicBitset::new(buffers.len());
        Self {
            borrow_marker,
            borrow_marker_mut,
            buffers,
        }
    }
}

impl<T: ?Sized> Storage<T> {
    pub fn new(len: usize, gen: impl Fn(usize) -> Box<T>) -> Self {
        let borrow_marker = AtomicBitset::new(len);
        let borrow_marker_mut = AtomicBitset::new(len);
        let buffers = (0..len).map(|i| UnsafeCell::new(gen(i))).collect::<Box<[_]>>();
        Self {
            borrow_marker,
            borrow_marker_mut,
            buffers,
        }
    }
}

impl<T> SharedStorage for Storage<[T]> {
    type Value = [T];

    fn len(&self) -> usize {
        self.buffers.len()
    }

    fn get(&self, index: usize) -> SRef<Self::Value> {
        self.get_buffer(index)
    }

    fn get_mut(&self, index: usize) -> SMut<Self::Value> {
        self.get_buffer_mut(index)
    }
}

impl SharedStorage for Storage<ParamBuffer> {
    type Value = ParamBuffer;

    fn len(&self) -> usize {
        self.buffers.len()
    }

    fn get(&self, index: usize) -> SRef<Self::Value> {
        self.get_buffer(index)
    }

    fn get_mut(&self, index: usize) -> SMut<Self::Value> {
        self.get_buffer_mut(index)
    }
}

impl SharedStorage for Storage<NoteBuffer> {
    type Value = NoteBuffer;

    fn len(&self) -> usize {
        self.buffers.len()
    }

    fn get(&self, index: usize) -> SRef<Self::Value> {
        self.get_buffer(index)
    }

    fn get_mut(&self, index: usize) -> SMut<Self::Value> {
        self.get_buffer_mut(index)
    }
}

impl<T: ?Sized> Storage<T> {
    fn get_buffer(&self, index: usize) -> SRef<T> {
        assert!(index < self.buffers.len());
        assert!(
            !self.borrow_marker_mut.get(index),
            "Buffer {index} is already mutably borrowed"
        );

        self.borrow_marker.set(index);
        let ptr = self.buffers[index].get();
        let slice = unsafe { &**ptr };

        SRef {
            data: slice,
            borrow_marker: self.borrow_marker.clone(),
            index,
        }
    }

    fn get_buffer_mut(&self, index: usize) -> SMut<T> {
        assert!(index < self.buffers.len());
        assert!(
            !self.borrow_marker_mut.get(index),
            "Buffer {index} is already mutably borrowed"
        );
        assert!(!self.borrow_marker.get(index), "Buffer {index} is already borrowed");

        self.borrow_marker_mut.set(index);
        let ptr = self.buffers[index].get();
        let slice = unsafe { &mut **ptr };

        SMut {
            data: slice,
            borrow_marker: self.borrow_marker.clone(),
            index,
        }
    }
}

pub struct MappedStorage<'a, S> {
    pub(crate) storage: S,
    pub(crate) index_map: &'a [usize],
}

impl<S: SharedStorage> SharedStorage for MappedStorage<'_, S> {
    type Value = S::Value;

    fn len(&self) -> usize {
        self.storage.len()
    }

    fn get(&self, index: usize) -> SRef<Self::Value> {
        let index = self.index_map[index];
        self.storage.get(index)
    }

    fn get_mut(&self, index: usize) -> SMut<Self::Value> {
        let index = self.index_map[index];
        self.storage.get_mut(index)
    }
}
