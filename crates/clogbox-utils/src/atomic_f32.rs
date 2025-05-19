//! Atomic operations for `f32` values.
//!
//! This module provides atomic operations for f32 values, which are useful for thread-safe communication between audio
//! processing and UI threads. They are implemented by transmuting `f32` values into `u32` and using [`AtomicU32`] as
//! the backing atomic type.
use std::sync::atomic::{AtomicU32, Ordering};

/// Atomic f32 container based on an [`AtomicU32`]. This type is `repr(transparent)`, which means, transitively, that
/// it is the same as a single `u32` (which in turn is the same as a single `f32`).
#[repr(transparent)]
#[derive(Debug, Default)]
pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    /// Create a new [`AtomicF32`] from the provided value.
    pub const fn new(value: f32) -> Self {
        Self(AtomicU32::new(value.to_bits()))
    }

    /// Unwraps this [`AtomicF32`] and returns the raw value.
    pub const fn into_inner(self) -> f32 {
        f32::from_bits(self.0.into_inner())
    }

    /// Loads the value of an atomic `f32` and returns it.
    ///
    /// This method retrieves the underlying `f32` value stored in the atomic instance by converting
    /// its raw bits, represented as a `u32`, into a `f32` using [`f32::from_bits`]. The load operation
    /// is performed atomically and adheres to the specified memory ordering.
    ///
    /// # Parameters
    ///
    /// * `order`: The memory ordering for the load operation, specified by the [`Ordering`] enum. Refer to the
    ///   documentation for [`Ordering`] for more details on these options.
    ///
    /// # Returns
    ///
    /// A `f32` representing the value stored in the atomic instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::atomic::Ordering;
    /// use clogbox_utils::AtomicF32;
    ///
    /// // Example atomic value wrapped in a helper structure
    /// let atomic_f32 = AtomicF32::new(1234.0); // Bit pattern for 1.0f32
    ///
    /// let value = atomic_f32.load(Ordering::SeqCst); // Loads the f32 value
    /// assert_eq!(value, 1.0);
    /// ```
    pub fn load(&self, order: Ordering) -> f32 {
        f32::from_bits(self.0.load(order))
    }

    /// Stores an `f32` value atomically into the underlying atomic storage.
    ///
    /// # Parameters
    ///
    /// - `value`: The `f32` value to store.
    /// - `order`: The memory ordering to be used for the atomic store. Possible orderings
    ///   are defined in the [`Ordering`] enum.
    ///
    /// # Behavior
    ///
    /// The provided `f32` value is first converted into its bit representation using
    /// the [`f32::to_bits`] method. This bit representation (as a `u32`) is then stored in
    /// the underlying atomic storage, adhering to the specified memory ordering.
    ///
    /// # Example
    /// ```
    /// use std::sync::atomic::Ordering;
    /// use clogbox_utils::AtomicF32;
    ///
    /// let atomic_value = AtomicF32::default();
    /// atomic_value.store(3.14, Ordering::SeqCst);
    /// assert_eq!(3.14, atomic_value.into_inner());
    /// ```
    pub fn store(&self, value: f32, order: Ordering) {
        self.0.store(value.to_bits(), order);
    }
}
