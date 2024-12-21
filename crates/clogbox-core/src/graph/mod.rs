use crate::graph::event::EventBuffer;
use crate::r#enum::Enum;
use derive_more::{Deref, DerefMut};
use duplicate::duplicate_item;
use num_traits::Zero;
use std::borrow::Cow;
use ordered_float::NotNan;
use typenum::U3;

pub mod context;
pub mod driver;
pub mod event;
mod r#impl;
pub mod module;
pub mod slots;
mod storage;

/// Enum of slot types that a module can export
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum SlotType {
    /// Audio-rate data
    Audio,
    /// Control-rate events
    Control,
    /// Note events
    Note,
}

impl az::Cast<usize> for SlotType {
    fn cast(self) -> usize {
        match self {
            SlotType::Audio => 0,
            SlotType::Control => 1,
            SlotType::Note => 2,
        }
    }
}

impl az::CastFrom<usize> for SlotType {
    fn cast_from(value: usize) -> Self {
        match value {
            0 => SlotType::Audio,
            1 => SlotType::Control,
            2 => SlotType::Note,
            _ => unreachable!(),
        }
    }
}

impl Enum for SlotType {
    type Count = U3;

    fn name(&self) -> Cow<str> {
        match self {
            SlotType::Audio => Cow::Borrowed("Audio"),
            SlotType::Control => Cow::Borrowed("Control"),
            SlotType::Note => Cow::Borrowed("Note"),
        }
    }
}

impl SlotType {
    /// Returns whether a slot of this type can be connected into a slot of the other type.
    pub fn connects_to(self, other: Self) -> bool {
        self == other
    }
}

/// Wrapper for timestamped values.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Deref, DerefMut)]
pub struct Timestamped<T> {
    /// Relative sample position to the point at which the process method is invoked.
    pub sample: usize,
    /// Inner value
    #[deref]
    #[deref_mut]
    pub value: T,
}

impl<T> Timestamped<T> {
    /// Copies the timestamp, but returns a reference to the value.
    pub fn as_ref(&self) -> Timestamped<&T> {
        Timestamped {
            sample: self.sample,
            value: &self.value,
        }
    }

    /// Copies the timestamp, but returns a mutable reference to the value.
    pub fn as_mut(&mut self) -> Timestamped<&mut T> {
        Timestamped {
            sample: self.sample,
            value: &mut self.value,
        }
    }

    /// Maps the timestamped value, keeping the timestamp.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Timestamped<U> {
        Timestamped {
            sample: self.sample,
            value: f(self.value),
        }
    }
}

pub type ControlBuffer = EventBuffer<f32>;
pub type NoteBuffer = EventBuffer<(NoteKey, NoteEvent)>;

#[duplicate_item(
    ty         reference(lifetime, type);
    [Slot]     [& 'lifetime type];
    [SlotMut]  [& 'lifetime mut type];
)]
#[derive(Debug)]
pub enum ty<'a, T> {
    /// Audio-rate buffer
    AudioBuffer(reference([a], [[T]])),
    /// Control-rate events buffer
    ControlEvents(reference([a], [ControlBuffer])),
    /// Note events buffer
    NoteEvents(reference([a], [NoteBuffer])),
}

#[duplicate_item(
    ty         reference(lifetime, type);
    [Slot]     [& 'lifetime type];
    [SlotMut]  [& 'lifetime mut type];
)]
impl<'a, T> ty<'a, T> {
    pub fn to_audio_buffer(self) -> Option<reference([a], [[T]])> {
        match self {
            Self::AudioBuffer(buf) => Some(buf),
            _ => None,
        }
    }

    pub fn to_control_events(self) -> Option<reference([a], [ControlBuffer])> {
        match self {
            Self::ControlEvents(events) => Some(events),
            _ => None,
        }
    }

    pub fn to_note_events(self) -> Option<reference([a], [NoteBuffer])> {
        match self {
            Self::NoteEvents(events) => Some(events),
            _ => None,
        }
    }
}

impl<'a, T: Zero> SlotMut<'a, T> {
    pub fn clear(&mut self) {
        match self {
            SlotMut::AudioBuffer(buf) => buf.fill_with(T::zero),
            SlotMut::ControlEvents(events) => {
                events.clear();
            }
            SlotMut::NoteEvents(events) => {
                events.clear();
            }
        }
    }
}

/// Type uniquely defining a note
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoteKey {
    pub channel: u8,
    pub note: u8,
}

/// Type of note events
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum NoteEvent {
    NoteOn { velocity: f32 },
    NoteOff { velocity: f32 },
    Pressure(f32),
    Timbre(f32),
    Pan(f32),
    Gain(f32),
}
