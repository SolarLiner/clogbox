use az::CastFrom;
use num_traits::Float;

/// Converts a MIDI note value to its corresponding frequency in Hertz (Hz).
///
/// # Parameters
///
/// - `midi_note`: An unsigned 8-bit integer (`u8`) representing the MIDI note number.
///   The MIDI specification defines note values in the range of 0 to 127, where:
///   - 69 corresponds to A4 (440 Hz).
///
/// # Returns
///
/// A `f64` value representing the frequency (in Hertz) of the given MIDI note.
///
/// # Additional notes
///
/// ## Formula
///
/// The function uses the formula:
///
/// $$ f = 440 \times \frac{2^{(n - 69)}{12} $$
///
/// where:
/// - $f$ is the frequency in Hz,
/// - $n$ is the MIDI note number,
/// - 69 is the MIDI note number for A4, the reference pitch.
/// This formula is derived from the equal-tempered scale used in Western music.
///
/// # Example
/// ```rust
/// use clogbox_math::frequency::midi_note_to_frequency;
///
/// let frequency = midi_note_to_frequency(69); // MIDI note 69 is A4
/// assert_eq!(frequency, 440.0);
///
/// let frequency_c4 = midi_note_to_frequency(60); // MIDI note 60 is Middle C (C4)
/// assert!((frequency_c4 - 261.63).abs() < 0.01); // Approximately 261.63 Hz
/// ```
///
/// # Notes
///
/// - This function assumes 12-tone equal temperament tuning (the most common modern tuning system).
pub fn midi_note_to_frequency(midi_note: u8) -> f64 {
    440.0 * 2.0f64.powf((midi_note as f64 - 69.0) / 12.0)
}

/// Converts a given frequency (in Hertz) to its corresponding MIDI note number.
///
/// MIDI note numbers are used in music software and hardware to represent musical notes.
/// The standard MIDI pitch number system assigns pitch number 69 to the note A4 (440 Hz).
///
/// # Arguments
///
/// * `frequency` - A `f64` representing the frequency in hertz (Hz) to be converted to a MIDI note.
///
/// # Returns
///
/// * A `f64` value representing the corresponding MIDI note number. The result is not rounded and can represent
///   fractional MIDI note numbers for frequencies between standard notes.
///
/// # Formula
///
/// The MIDI note number is calculated using the formula:
/// $$ 69 + 12 \times \log_2\left(\frac{frequency}{440}\right) $$
///
/// - $69$ represents the MIDI note number for A4 (440 Hz).
/// - $12$ represents the number of semitones in an octave.
///
/// # Examples
///
/// ```
/// use clogbox_math::frequency::frequency_to_midi_note;
///
/// let frequency = 440.0; // A4
/// let midi_note = frequency_to_midi_note(frequency);
/// assert_eq!(midi_note, 69.0);
///
/// let frequency = 880.0; // A5
/// let midi_note = frequency_to_midi_note(frequency);
/// assert_eq!(midi_note, 81.0);
///
/// let frequency = 261.63; // C4 (Middle C)
/// let midi_note = frequency_to_midi_note(frequency);
/// assert_eq!(midi_note, 60.0);
/// ```
#[numeric_literals::replace_float_literals(T::cast_from(literal))]
pub fn frequency_to_midi_note<T: CastFrom<f64> + Float>(frequency: T) -> T {
    69.0 + 12.0 * (frequency / 440.0).log2()
}

/// Converts an octave interval into a frequency ratio.
///
/// # Arguments
///
/// * `octave` - A value of type `T` representing the number of octaves.
///              This can be a fractional value for non-integer octave intervals.
///
/// # Returns
///
/// * A value of type `T` representing the corresponding frequency ratio.
///   For example,
///   - An octave value of `0` returns `1.0` (no change in frequency).
///   - An octave value of `1` returns `2.0` (doubling the frequency).
///   - An octave value of `-1` returns `0.5` (halving the frequency).
///
/// # Type Parameters
///
/// * `T` - The numeric type used for the operation. It must implement the `CastFrom<f64>`
///         and `Float` traits to support casting and floating-point calculations.
///
/// # Example
///
/// ```
/// use clogbox_math::frequency::octave_to_ratio;
///
/// let ratio: f64 = octave_to_ratio(1.0);  // Corresponds to 2^1 = 2.0
/// assert_eq!(ratio, 2.0);
///
/// let ratio: f64 = octave_to_ratio(0.0);  // Corresponds to 2^0 = 1.0
/// assert_eq!(ratio, 1.0);
///
/// let ratio: f64 = octave_to_ratio(-1.0); // Corresponds to 2^-1 = 0.5
/// assert_eq!(ratio, 0.5);
/// ```
///
/// # Notes
///
/// This function is generic over types implementing the `CastFrom<f64>` and
/// `Float` traits. This allows it to work with various numeric types such as
/// `f32`, `f64`, or custom numeric types that support these traits.
pub fn octave_to_ratio<T: CastFrom<f64> + Float>(octave: T) -> T {
    T::cast_from(2.0).powf(octave)
}

/// Converts a given number of semitones into a frequency ratio.
///
/// This function takes a value representing a number of semitones and converts it into
/// the corresponding frequency ratio, using the formula:
///
/// $$ ratio = 2^{\frac{semitones}{12}} $$
///
/// # Example
///
/// ```
/// use clogbox_math::frequency::semitones_to_ratio;
///
/// let semitones = 12.0; // One octave
/// let ratio = semitones_to_ratio(semitones);
/// assert_eq!(ratio, 2.0); // One octave equals a ratio of 2:1
/// ```
#[numeric_literals::replace_float_literals(T::cast_from(literal))]
pub fn semitones_to_ratio<T: CastFrom<f64> + Float>(semitones: T) -> T {
    octave_to_ratio(semitones / 12.0)
}

/// Converts a ratio to its equivalent value in octaves.
///
/// The function takes a ratio, typically representing a musical frequency ratio,
/// and computes its logarithm base 2. The result corresponds to the number
/// of octaves represented by the given ratio.
///
/// # Arguments
///
/// * `ratio` - A floating-point number (`T`) that represents the ratio.
///    It must be a positive non-zero value, as the logarithm is undefined for
///    zero or negative numbers. The type `T` is constrained by the `Float` trait.
///
/// # Returns
///
/// A floating-point number (`T`) representing the value in octaves.
///
/// # Example
///
/// ```
/// use clogbox_math::frequency::ratio_to_octave;
///
/// let ratio = 2.0; // A ratio of 2 represents one octave
/// let octaves = ratio_to_octave(ratio);
/// assert_eq!(octaves, 1.0);
///
/// let ratio = 4.0; // A ratio of 4 represents two octaves
/// let octaves = ratio_to_octave(ratio);
/// assert_eq!(octaves, 2.0);
/// ```
pub fn ratio_to_octave<T: Float>(ratio: T) -> T {
    debug_assert!(ratio > T::zero());
    ratio.log2()
}

/// Converts a given frequency ratio into its corresponding number of semitones.
///
/// # Arguments
///
/// - `ratio`: The frequency ratio (of type `T`) to be converted. Typically,
///   this represents the ratio between two frequencies (e.g., `2.0` for an octave).
///
/// # Example
///
/// ```
/// use clogbox_math::frequency::ratio_to_semitones;
///
/// // Assuming `f64` is used as the concrete type for `T`.
/// let ratio = 2.0; // An octave corresponds to a ratio of 2:1.
/// let semitones = ratio_to_semitones::<f64>(ratio);
/// assert_eq!(semitones, 12.0); // One octave equals 12 semitones.
/// ```
#[numeric_literals::replace_float_literals(T::cast_from(literal))]
pub fn ratio_to_semitones<T: CastFrom<f64> + Float>(ratio: T) -> T {
    ratio * 12.0
}
