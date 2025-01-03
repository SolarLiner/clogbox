use crate::graph::event::EventBuffer;
use crate::graph::{ControlBuffer, NoteBuffer, Slot, SlotMut, SlotType};
use clogbox_enum::enum_map::EnumMapArray;
use derive_more::{Deref, DerefMut};
use num_traits::Zero;
use std::cell::UnsafeCell;
use std::fmt;
use std::fmt::{Formatter, Write};
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

struct AtomicBitset {
    bits: Vec<AtomicU64>,
}

impl fmt::Debug for AtomicBitset {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
            bits: std::iter::repeat_with(|| AtomicU64::new(0))
                .take(len)
                .collect(),
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

pub struct GraphStorage<T> {
    audio_buffers: Vec<UnsafeCell<Box<[T]>>>,
    control_events: Vec<UnsafeCell<ControlBuffer>>,
    note_events: Vec<UnsafeCell<NoteBuffer>>,
    shared_borrows: Arc<EnumMapArray<SlotType, AtomicBitset>>,
    mut_borrows: Arc<EnumMapArray<SlotType, AtomicBitset>>,
}

impl<T> fmt::Debug for GraphStorage<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("GraphStorage").finish_non_exhaustive()
    }
}

impl<T> GraphStorage<T> {
    pub(crate) fn get_buffer(&self, slot_type: SlotType, slot_index: usize) -> SlotRef<T> {
        assert!(
            !self.mut_borrows[slot_type].get(slot_index),
            "Buffer is already mutably borrowed"
        );
        // # Safety
        //
        // Borrows have been checked above, which means the mutable borrow cannot induce aliasing.
        let slot = match slot_type {
            SlotType::Audio => Slot::AudioBuffer({
                let ptr = self.audio_buffers[slot_index].get();
                unsafe { &*ptr }.iter().as_slice()
            }),
            SlotType::Control => Slot::ControlEvents({
                let ptr = self.control_events[slot_index].get();
                unsafe { &*ptr }
            }),
            SlotType::Note => Slot::NoteEvents({
                let ptr = self.note_events[slot_index].get();
                unsafe { &*ptr }
            }),
        };

        self.shared_borrows[slot_type].set(slot_index);
        SlotRef {
            data: slot,
            slot_type,
            index: slot_index,
            borrow_marker: self.shared_borrows.clone(),
        }
    }

    pub(crate) fn get_buffer_mut(&self, slot_type: SlotType, slot_index: usize) -> SlotRefMut<T> {
        assert!(
            !self.shared_borrows[slot_type].get(slot_index),
            "Buffer is already borrowed"
        );
        assert!(
            !self.mut_borrows[slot_type].get(slot_index),
            "Buffer is already mutably borrowed"
        );
        // # Safety
        //
        // Borrows have been checked above, which means the mutable borrow cannot induce aliasing.
        let slot_mut = match slot_type {
            SlotType::Audio => SlotMut::AudioBuffer({
                let ptr = self.audio_buffers[slot_index].get();
                let ref_ = unsafe { &mut *ptr };
                &mut *ref_
            }),
            SlotType::Control => SlotMut::ControlEvents({
                let ptr = self.control_events[slot_index].get();
                unsafe { &mut *ptr }
            }),
            SlotType::Note => SlotMut::NoteEvents({
                let ptr = self.note_events[slot_index].get();
                unsafe { &mut *ptr }
            }),
        };
        SlotRefMut {
            data: slot_mut,
            slot_type,
            index: slot_index,
            borrow_marker: self.mut_borrows.clone(),
        }
    }
}

impl<T: Zero> GraphStorage<T> {
    pub(crate) fn new(
        max_block_size: usize,
        num_audio_buffers: usize,
        num_control_events: usize,
        num_note_events: usize,
    ) -> Self {
        let gen_bitset = || {
            Arc::new(EnumMapArray::new(|typ| match typ {
                SlotType::Audio => AtomicBitset::new(num_audio_buffers),
                SlotType::Control => AtomicBitset::new(num_control_events),
                SlotType::Note => AtomicBitset::new(num_note_events),
            }))
        };
        Self {
            audio_buffers: (0..num_audio_buffers)
                .map(|_| (0..max_block_size).map(|_| T::zero()).collect())
                .map(UnsafeCell::new)
                .collect(),
            control_events: (0..num_control_events)
                .map(|_| EventBuffer::new(max_block_size))
                .map(UnsafeCell::new)
                .collect(),
            note_events: (0..num_note_events)
                .map(|_| EventBuffer::new(max_block_size))
                .map(UnsafeCell::new)
                .collect(),
            shared_borrows: gen_bitset(),
            mut_borrows: gen_bitset(),
        }
    }
}
pub type SlotRef<'a, T> = StorageBorrow<Slot<'a, T>>;
pub type SlotRefMut<'a, T> = StorageBorrow<SlotMut<'a, T>>;

#[derive(Debug, Deref, DerefMut)]
pub struct StorageBorrow<T> {
    #[deref]
    #[deref_mut]
    pub data: T,
    slot_type: SlotType,
    index: usize,
    borrow_marker: Arc<EnumMapArray<SlotType, AtomicBitset>>,
}

impl<T> StorageBorrow<T> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> StorageBorrow<U> {
        let this = ManuallyDrop::new(self);
        StorageBorrow {
            // # Safety
            //
            // `data` is never read from/written to again (self has been move into this method, and
            // `ManuallyDrop` will ensure that no matter the drop implementation, it just won't
            // be called). Rust cannot recognize that this is a valid move, so we force its hand
            // here.
            data: f(unsafe { std::ptr::read(&this.data) }),
            slot_type: this.slot_type,
            index: this.index,
            borrow_marker: this.borrow_marker.clone(),
        }
    }
}

impl<T> StorageBorrow<Option<T>> {
    pub fn transpose(self) -> Option<StorageBorrow<T>> {
        if self.data.is_none() {
            None
        } else {
            Some(self.map(|x| x.unwrap()))
        }
    }
}

impl<T> Drop for StorageBorrow<T> {
    fn drop(&mut self) {
        self.borrow_marker[self.slot_type].clear(self.index);
    }
}
