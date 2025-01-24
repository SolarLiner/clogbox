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
