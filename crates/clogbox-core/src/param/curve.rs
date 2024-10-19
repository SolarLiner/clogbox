//! This module provides utilities for handling parameter curves with optional smoothing capabilities.
//!
//! The main structure allows adding and retrieving parametric values either by sample index or time in seconds.
//! It also supports optional smoothing for transitions between values to accommodate different sample rates.
//!
//! # Example
//!
//! ```rust
//! use clogbox_core::param::curve::ParamCurve;
//!
//! let mut curve = ParamCurve::new(44100.0, 100, 0.0);
//! curve.add_value_seconds(1.0, 1.0);
//! let value = curve.get_value_seconds(0.5);
//! println!("Value at 0.5 seconds: {}", value);
//! ```
#[derive(Debug, Copy, Clone)]
struct Smoother {
    sample_rate: f32,
    max_rate: f32,
    max_sample_diff: f32,
}

impl Smoother {
    fn new(sample_rate: f32, max_rate: f32) -> Self {
        Self {
            sample_rate,
            max_rate,
            max_sample_diff: max_rate / sample_rate,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.max_sample_diff = self.max_rate / self.sample_rate;
    }

    fn smooth(&self, sample_rate: f32, seconds: f32, a: f32, b: f32) -> f32 {
        let max_diff = self.max_sample_diff * seconds * sample_rate;
        let diff = b - a;
        a + diff.clamp(-max_diff, max_diff)
    }
}

/// A `ParamCurve` holds a set of timestamped values and allows for
/// interpolation and smoothing of values between timestamps.
#[derive(Debug, Clone)]
pub struct ParamCurve {
    initial_value: f32,
    timestamps: Vec<(f32, f32)>,
    sample_rate: f32,
    smoother: Option<Smoother>,
}

impl ParamCurve {
    /// Creates a new `ParamCurve` with the specified sample rate, initial value,
    /// and capacity for a given number of timestamps.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - The sample rate (in Hz) used to interpret the timestamps.
    /// * `max_timestamps` - The maximum number of timestamps the curve can store.
    /// * `initial_value` - The initial value before any timestamps are added.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::param::curve::ParamCurve;
    /// let param_curve = ParamCurve::new(44100.0, 10, 0.5);
    /// ```
    pub fn new(sample_rate: f32, max_timestamps: usize, initial_value: f32) -> Self {
        Self {
            initial_value,
            timestamps: Vec::with_capacity(max_timestamps),
            sample_rate,
            smoother: None,
        }
    }

    /// Returns the current sample rate of the `ParamCurve`.
    ///
    /// # Returns
    ///
    /// The sample rate as a `f32`.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::param::curve::ParamCurve;
    /// let param_curve = ParamCurve::new(44100.0, 10, 0.5);
    /// assert_eq!(44100.0, param_curve.sample_rate());
    /// ```
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Sets a new sample rate for the `ParamCurve`. If a smoother is set,
    /// the smoother's sample rate will also be updated.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - The new sample rate (in Hz) to set.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::param::curve::ParamCurve;
    /// let mut param_curve = ParamCurve::new(44100.0, 10, 0.5);
    /// param_curve.set_sample_rate(48000.0);
    /// ```
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        if let Some(smoother) = &mut self.smoother {
            smoother.set_sample_rate(sample_rate);
        }
    }

    /// Sets an optional `Smoother` for the `ParamCurve`, allowing for interpolation
    /// between values at different timestamps.
    ///
    /// # Arguments
    ///
    /// * `max_rate` - Maximum rate of change for the given parameter
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::param::curve::ParamCurve;
    /// let mut param_curve = ParamCurve::new(44100.0, 10, 0.5);
    /// param_curve.set_smoother(10.);
    /// ```
    pub fn set_smoother(&mut self, max_rate: f32) {
        self.smoother = Some(Smoother::new(self.sample_rate, max_rate));
    }

    /// Clears the smoother from the `ParamCurve`, if any was set.
    ///
    /// After calling this method, no interpolation or smoothing will occur between
    /// timestamps, and the curve will return the raw values as they are.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::param::curve::ParamCurve;
    /// let mut param_curve = ParamCurve::new(44100.0, 10, 0.5);
    /// param_curve.set_smoother(10.);
    ///
    /// // Later, if you want to remove the smoother:
    /// param_curve.clear_smoother();
    /// ```
    pub fn clear_smoother(&mut self) {
        self.smoother.take();
    }

    /// Sets an optional `Smoother` and returns the `ParamCurve` instance, allowing
    /// for method chaining. The smoother is used for interpolating between values.
    ///
    /// # Arguments
    ///
    /// * `max_rate` - Maximum rate of change for the given parameter
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::param::curve::ParamCurve;
    /// let mut param_curve = ParamCurve::new(44100.0, 10, 0.5).with_smoother(10.);
    /// ```
    pub fn with_smoother(mut self, max_rate: f32) -> Self {
        self.set_smoother(max_rate);
        self
    }

    /// Clears all the timestamps from the `ParamCurve`, resetting it to an empty state.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::param::curve::ParamCurve;
    /// let mut param_curve = ParamCurve::new(44100.0, 10, 0.5);
    /// param_curve.clear();
    /// ```
    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    /// Adds a value to the curve at the given sample index, interpreting the sample
    /// index as a time (in samples) based on the sample rate.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp (in samples) to associate with the value.
    /// * `value` - The value to add to the curve.
    ///
    /// # Returns
    ///
    /// `true` if the value was added successfully, `false` if the curve has reached
    /// its capacity for storing timestamps.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::param::curve::ParamCurve;
    /// let mut param_curve = ParamCurve::new(44100.0, 10, 0.5);
    /// let success = param_curve.add_value_sample(44100, 1.0);  // Adds value at 1 second
    /// assert_eq!(1.0, param_curve.get_value_sample(44100));
    /// ```
    pub fn add_value_sample(&mut self, timestamp: usize, value: f32) -> bool {
        self.add_value_seconds(timestamp as f32 / self.sample_rate, value)
    }

    /// Adds a value to the curve at a specific time (in seconds).
    ///
    /// # Arguments
    ///
    /// * `seconds` - The time (in seconds) at which the value is added.
    /// * `value` - The value to add to the curve.
    ///
    /// # Returns
    ///
    /// `true` if the value was added successfully, `false` if the curve has reached
    /// its capacity for storing timestamps.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::param::curve::ParamCurve;
    /// let mut param_curve = ParamCurve::new(44100.0, 10, 0.5);
    /// assert!(param_curve.add_value_seconds(1.0, 1.0));  // Adds value at 1 second
    /// assert_eq!(1.0, param_curve.get_value_seconds(1.0));
    /// ```
    pub fn add_value_seconds(&mut self, seconds: f32, value: f32) -> bool {
        if self.timestamps.len() == self.timestamps.capacity() {
            return false;
        }
        self.timestamps.push((seconds, value));
        true
    }

    /// Retrieves the value at a specific sample index, interpolating between values
    /// if necessary.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp (in samples) for which to retrieve the value.
    ///
    /// # Returns
    ///
    /// The interpolated value at the given timestamp.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::param::curve::ParamCurve;
    /// let param_curve = ParamCurve::new(44100.0, 10, 0.5);
    /// assert_eq!(0.5, param_curve.get_value_sample(44100));  // Retrieves the value at 1 second
    /// ```
    pub fn get_value_sample(&self, timestamp: usize) -> f32 {
        self.get_value_seconds(timestamp as f32 / self.sample_rate)
    }

    /// Retrieves the value at a specific time (in seconds), interpolating between values
    /// if necessary.
    ///
    /// # Arguments
    ///
    /// * `seconds` - The time (in seconds) for which to retrieve the value.
    ///
    /// # Returns
    ///
    /// The interpolated value at the given time.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::param::curve::ParamCurve;
    /// let param_curve = ParamCurve::new(44100.0, 10, 0.5);
    /// let value = param_curve.get_value_seconds(1.0);  // Retrieves the value at 1 second
    /// ```
    pub fn get_value_seconds(&self, seconds: f32) -> f32 {
        let result = self
            .timestamps
            .binary_search_by(|(pos, _)| pos.total_cmp(&seconds));
        match result {
            Ok(pos) => self.timestamps[pos].1,
            Err(0) => self.initial_value,
            Err(insert) if insert < self.timestamps.len() => {
                let (p1, v1) = self.timestamps[insert - 1];
                if let Some(smoother) = &self.smoother {
                    let (_, v2) = self.timestamps[insert];
                    smoother.smooth(self.sample_rate, seconds - p1, v1, v2)
                } else {
                    v1
                }
            }
            Err(_) => self.last_value(),
        }
    }

    /// Returns the most recent value added to the `ParamCurve`.
    ///
    /// If no values have been added, it returns the initial value that was provided
    /// when the curve was created.
    ///
    /// # Returns
    ///
    /// The last value in the curve, or the initial value if the curve is empty.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::param::curve::ParamCurve;
    /// let mut param_curve = ParamCurve::new(44100.0, 10, 0.5);
    /// assert_eq!(param_curve.last_value(), 0.5); // Initial value, as no timestamps are added yet
    ///
    /// param_curve.add_value_sample(0, 1.0);
    /// assert_eq!(param_curve.last_value(), 1.0); // Last added value
    /// ```
    pub fn last_value(&self) -> f32 {
        self.timestamps
            .last()
            .map(|(_, value)| *value)
            .unwrap_or(self.initial_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn test_add_value_sample() {
        let mut param_curve = ParamCurve::new(44100.0, 3, 0.0);

        // Add value samples at specific timestamps
        assert!(param_curve.add_value_sample(0, 0.5));
        assert!(param_curve.add_value_sample(44100, 1.0)); // 1 second
        assert!(param_curve.add_value_sample(88200, 1.5)); // 2 seconds

        // Ensure the values were added correctly
        assert_eq!(param_curve.get_value_sample(0), 0.5);
        assert_eq!(param_curve.get_value_sample(44100), 1.0);
        assert_eq!(param_curve.get_value_sample(88200), 1.5);
    }

    #[rstest]
    fn test_add_value_seconds() {
        let mut param_curve = ParamCurve::new(44100.0, 3, 0.0);

        // Add values using seconds
        assert!(param_curve.add_value_seconds(0.0, 0.5));
        assert!(param_curve.add_value_seconds(1.0, 1.0));
        assert!(param_curve.add_value_seconds(2.0, 1.5));

        // Ensure the values were added correctly
        assert_eq!(param_curve.get_value_seconds(0.0), 0.5);
        assert_eq!(param_curve.get_value_seconds(1.0), 1.0);
        assert_eq!(param_curve.get_value_seconds(2.0), 1.5);
    }

    #[rstest]
    fn test_interpolated_value_no_smoother() {
        let mut param_curve = ParamCurve::new(44100.0, 3, 0.0);

        // Add values at specific seconds
        assert!(param_curve.add_value_seconds(0.0, 0.5));
        assert!(param_curve.add_value_seconds(2.0, 1.5));

        // Read value between known timestamps
        let interpolated_value = param_curve.get_value_seconds(1.0);

        // Since no smoother is set, it should return the first value
        assert_eq!(interpolated_value, 0.5);
    }

    #[rstest]
    fn test_out_of_bounds_value() {
        let mut param_curve = ParamCurve::new(44100.0, 2, 0.0);

        // Add values at specific seconds
        assert!(param_curve.add_value_seconds(0.0, 0.5));
        assert!(param_curve.add_value_seconds(1.0, 1.0));

        // Access a value beyond the known timestamps
        // Before start, it should return the initial value
        let value_before_start = param_curve.get_value_seconds(-1.0); // Before first value
        assert_eq!(value_before_start, 0.0);

        let value_after_end = param_curve.get_value_seconds(2.0); // After last value

        // It should return the last value
        assert_eq!(value_after_end, 1.0);
    }

    #[rstest]
    fn test_add_value_sample_full() {
        // Create a ParamCurve with capacity for 2 timestamps
        let mut param_curve = ParamCurve::new(44100.0, 2, 0.0);

        // Add values until the capacity is reached
        assert!(param_curve.add_value_sample(0, 0.5)); // First value
        assert!(param_curve.add_value_sample(44100, 1.0)); // Second value

        // Adding another value should return false since capacity is reached
        assert!(!param_curve.add_value_sample(88200, 1.5));
    }

    #[rstest]
    fn test_add_value_seconds_full() {
        // Create a ParamCurve with capacity for 2 timestamps
        let mut param_curve = ParamCurve::new(44100.0, 2, 0.0);

        // Add values using seconds until the capacity is reached
        assert!(param_curve.add_value_seconds(0.0, 0.5)); // First value
        assert!(param_curve.add_value_seconds(1.0, 1.0)); // Second value

        // Adding another value should return false since capacity is reached
        assert!(!param_curve.add_value_seconds(2.0, 1.5));
    }
}
