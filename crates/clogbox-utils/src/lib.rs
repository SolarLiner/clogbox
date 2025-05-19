//! # `clogbox_utils`
//!
//! Utility types and functions for the `clogbox` audio framework.
//!
//! This crate provides various utility types and functions that are used
//! across the `clogbox` framework, including atomic operations for audio
//! processing.

#![warn(missing_docs)]

pub mod atomic_f32;

/// Re-export all items from the atomic_f32 module for convenience.
pub use atomic_f32::*;
