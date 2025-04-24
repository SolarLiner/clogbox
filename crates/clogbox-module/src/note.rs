//! Types for representing and manipulating MIDI-like note events.
//!
//! This module provides structures for working with musical note events,
//! such as note on/off events, with associated metadata like velocity,
//! frequency, and channel information.

use std::fmt;

/// Identifies a musical note by its channel and number.
///
/// This provides a unique identification for a note in a multi-channel
/// environment, similar to MIDI notes. Notes are ordered first by channel,
/// then by note number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoteId {
    /// The channel number, typically in the range 0-15 for MIDI compatibility.
    pub channel: u8,

    /// The note number, typically in the range 0-127 for MIDI compatibility.
    pub number: u8,
}

impl NoteId {
    /// Creates a new note identifier from a note number and channel.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::note::NoteId;
    /// // Create an identifier for note C4 (MIDI note 60) on channel 0
    /// let note_id = NoteId::new(60, 0);
    /// ```
    pub fn new(number: u8, channel: u8) -> Self {
        Self { channel, number }
    }

    /// Converts a note number to its frequency in Hz, using the standard
    /// equal temperament tuning with A4 (note 69) = 440Hz.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::note::NoteId;
    /// let middle_c = NoteId::new(60, 0);
    /// let freq = middle_c.to_frequency();
    /// assert!((freq - 261.6256).abs() < 0.001); // ~261.63 Hz
    /// ```
    pub fn to_frequency(&self) -> f32 {
        // A4 (MIDI note 69) = 440 Hz
        // Each semitone is a factor of 2^(1/12)
        const A4_NOTE_NUMBER: f32 = 69.0;
        const A4_FREQUENCY: f32 = 440.0;
        const SEMITONE_RATIO: f32 = 1.059_463_1; // 2^(1/12)

        let semitones_from_a4 = self.number as f32 - A4_NOTE_NUMBER;
        A4_FREQUENCY * SEMITONE_RATIO.powf(semitones_from_a4)
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Format note names (C4, D#3, etc.)
        const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        let octave = (self.number / 12) as i32 - 1; // C0 is MIDI note 12
        let note_index = (self.number % 12) as usize;

        write!(f, "{}{} (ch:{})", NOTE_NAMES[note_index], octave, self.channel)
    }
}

// PartialOrd and Ord are now derived

/// Represents different types of note events with associated metadata.
///
/// Each variant includes the note identification (number and channel),
/// along with additional data specific to the event type.
#[derive(Debug, Clone, Copy)]
pub enum NoteEvent {
    /// A note-on event, indicating a note has started playing.
    NoteOn {
        /// The note identification (number and channel)
        id: NoteId,

        /// The frequency of the note in Hz
        frequency: f32,

        /// The velocity of the note, in the range 0.0 to 1.0
        velocity: f32,
    },

    /// A note-off event, indicating a note has stopped playing.
    NoteOff {
        /// The note identification (number and channel)
        id: NoteId,

        /// The frequency of the note in Hz
        frequency: f32,

        /// The release velocity, in the range 0.0 to 1.0 (often ignored)
        velocity: f32,
    },
}

impl NoteEvent {
    /// Creates a new note-on event with the given parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::note::{NoteId, NoteEvent};
    /// // Create a note-on event for middle C (MIDI note 60) on channel 0 with velocity 0.8
    /// let note_on = NoteEvent::note_on(60, 0, 0.8);
    /// ```
    pub fn note_on(number: u8, channel: u8, velocity: f32) -> Self {
        let id = NoteId::new(number, channel);
        let frequency = id.to_frequency();

        NoteEvent::NoteOn {
            id,
            frequency,
            velocity: velocity.clamp(0.0, 1.0),
        }
    }

    /// Creates a new note-off event with the given parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// # use clogbox_module::note::{NoteId, NoteEvent};
    /// // Create a note-off event for middle C (MIDI note 60) on channel 0
    /// let note_off = NoteEvent::note_off(60, 0, 0.5);
    /// ```
    pub fn note_off(number: u8, channel: u8, velocity: f32) -> Self {
        let id = NoteId::new(number, channel);
        let frequency = id.to_frequency();

        NoteEvent::NoteOff {
            id,
            frequency,
            velocity: velocity.clamp(0.0, 1.0),
        }
    }

    /// Returns the note identifier for this event.
    pub fn note_id(&self) -> NoteId {
        match self {
            NoteEvent::NoteOn { id, .. } => *id,
            NoteEvent::NoteOff { id, .. } => *id,
        }
    }

    /// Returns the frequency of this note event in Hz.
    pub fn frequency(&self) -> f32 {
        match self {
            NoteEvent::NoteOn { frequency, .. } => *frequency,
            NoteEvent::NoteOff { frequency, .. } => *frequency,
        }
    }

    /// Returns the velocity of this note event (in the range 0.0 to 1.0).
    pub fn velocity(&self) -> f32 {
        match self {
            NoteEvent::NoteOn { velocity, .. } => *velocity,
            NoteEvent::NoteOff { velocity, .. } => *velocity,
        }
    }

    /// Returns true if this is a note-on event.
    pub fn is_note_on(&self) -> bool {
        matches!(self, NoteEvent::NoteOn { .. })
    }

    /// Returns true if this is a note-off event.
    pub fn is_note_off(&self) -> bool {
        matches!(self, NoteEvent::NoteOff { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_id_frequency() {
        let a4 = NoteId::new(69, 0);
        assert!((a4.to_frequency() - 440.0).abs() < 0.001);

        let c4 = NoteId::new(60, 0);
        assert!((c4.to_frequency() - 261.6256).abs() < 0.001);
    }

    #[test]
    fn test_note_events() {
        let note_on = NoteEvent::note_on(60, 1, 0.8);
        let note_off = NoteEvent::note_off(60, 1, 0.5);

        match note_on {
            NoteEvent::NoteOn { id, velocity, .. } => {
                assert_eq!(id.number, 60);
                assert_eq!(id.channel, 1);
                assert_eq!(velocity, 0.8);
            }
            _ => panic!("Expected NoteOn variant"),
        }

        match note_off {
            NoteEvent::NoteOff { id, velocity, .. } => {
                assert_eq!(id.number, 60);
                assert_eq!(id.channel, 1);
                assert_eq!(velocity, 0.5);
            }
            _ => panic!("Expected NoteOff variant"),
        }
    }

    #[test]
    fn test_note_id_display() {
        let c4 = NoteId::new(60, 0);
        assert_eq!(c4.to_string(), "C4 (ch:0)");

        let fs2 = NoteId::new(42, 3);
        assert_eq!(fs2.to_string(), "F#2 (ch:3)");
    }

    #[test]
    fn test_note_id_ordering() {
        // Channel has higher precedence than note number
        let c4_ch0 = NoteId::new(60, 0);
        let c3_ch1 = NoteId::new(48, 1);
        assert!(c4_ch0 < c3_ch1); // Channel 0 comes before channel 1

        // When channels are the same, note numbers determine order
        let c4_ch1 = NoteId::new(60, 1);
        let e4_ch1 = NoteId::new(64, 1);
        assert!(c4_ch1 < e4_ch1); // C4 is lower than E4

        // Verify that a collection of NoteIds gets sorted correctly
        let mut notes = [
            NoteId::new(64, 1), // E4, ch 1
            NoteId::new(60, 0), // C4, ch 0
            NoteId::new(67, 0), // G4, ch 0
            NoteId::new(60, 1),
        ];

        notes.sort();

        assert_eq!(notes[0], NoteId::new(60, 0)); // C4, ch 0
        assert_eq!(notes[1], NoteId::new(67, 0)); // G4, ch 0
        assert_eq!(notes[2], NoteId::new(60, 1)); // C4, ch 1
        assert_eq!(notes[3], NoteId::new(64, 1)); // E4, ch 1
    }
}
