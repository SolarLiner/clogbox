use crate::graph::event::EventBuffer;
use clogbox_enum::Enum;
use derive_more::{Deref, DerefMut};
use duplicate::duplicate_item;
use num_traits::Zero;

pub mod context;
pub mod driver;
pub mod event;
mod r#impl;
pub mod module;
pub mod slots;
mod storage;

/// Enum of slot types that a module can export
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Enum)]
pub enum SlotType {
    /// Audio-rate data
    Audio,
    /// Control-rate events
    Control,
    /// Note events
    Note,
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

/// Type alias for parameter events (ie. at "control rate").
pub type ControlBuffer = EventBuffer<f32>;

/// Type alias for note events.
pub type NoteBuffer = EventBuffer<(NoteKey, NoteEvent)>;

#[duplicate_item(
    ty         reference(lifetime, type);
    [Slot]     [& 'lifetime type];
    [SlotMut]  [& 'lifetime mut type];
)]
#[derive(Debug)]
/// Runtime-variable buffer type for a given slot.
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
    /// Casts this slot to an audio buffer, if it is the right type.
    pub fn to_audio_buffer(self) -> Option<reference([a], [[T]])> {
        match self {
            Self::AudioBuffer(buf) => Some(buf),
            _ => None,
        }
    }

    /// Casts this slot to a control buffer, if it is the right type.
    pub fn to_control_events(self) -> Option<reference([a], [ControlBuffer])> {
        match self {
            Self::ControlEvents(events) => Some(events),
            _ => None,
        }
    }

    /// Casts this slot to a note buffer, if it is the right type.
    pub fn to_note_events(self) -> Option<reference([a], [NoteBuffer])> {
        match self {
            Self::NoteEvents(events) => Some(events),
            _ => None,
        }
    }
}

impl<'a, T: Zero> SlotMut<'a, T> {
    /// Clear the contents of this slot. For audio buffers, this means filling the buffer with
    /// zeros, otherwise it means removing all events.
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
    /// Noce channel
    pub channel: u8,
    /// Note key index
    pub note: u8,
}

/// Type of note events
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum NoteEvent {
    /// A key was pressed
    NoteOn {
        /// Velocity of the keypress
        velocity: f32,
    },
    /// A key was released
    NoteOff {
        /// Velocity of the release
        velocity: f32,
    },
    /// Per-key pressure (aka. Polyphonic Aftertouch)
    Pressure(f32),
    /// MPE Timbre
    Timbre(f32),
    /// MPE Pan
    Pan(f32),
    /// MPE Gain
    Gain(f32),
}
