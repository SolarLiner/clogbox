//! # `clogbox` Module
//!
//! Core abstractions for audio processing modules in the `clogbox` framework.
//!
//! This crate provides the fundamental types and traits for building modular
//! audio processing systems. It defines the `Module` trait, which is the central
//! abstraction for audio processing components, along with supporting types for
//! handling audio samples, parameters, and MIDI notes.

#![warn(missing_docs)]

use crate::eventbuffer::EventSlice;
use clogbox_enum::Enum;
use clogbox_math::recip::Recip;
use context::ProcessContext;
use std::num::NonZeroU32;

/// Context types for audio processing.
///
/// This module provides context structures that are passed to modules during processing,
/// containing information about the current processing state.
pub mod context;

/// Contributed modules and utilities.
///
/// This module contains additional modules and utilities contributed to the framework
/// that may be useful for audio processing.
pub mod contrib;

/// Dynamic module implementations.
///
/// This module provides dynamic dispatch wrappers for modules, allowing for runtime
/// polymorphism in audio processing graphs.
pub mod r#dyn;

/// Event buffer implementation for parameter and note events.
///
/// This module provides a buffer structure for storing and processing time-stamped events
/// such as parameter changes and MIDI notes.
pub mod eventbuffer;

/// Macros for simplifying module implementation.
///
/// This module contains macros that reduce boilerplate when implementing modules
/// and related types.
pub mod macros;

/// Common module implementations.
///
/// This module provides implementations of frequently used audio processing modules
/// such as gain, pan, and mixers.
pub mod modules;

/// MIDI note event handling.
///
/// This module provides types and utilities for working with MIDI note events
/// in audio processing modules.
pub mod note;

/// Sample-by-sample processing abstractions.
///
/// This module provides traits and types for implementing modules that process
/// audio one sample at a time, as opposed to block processing.
pub mod sample;

/// Sample rate representation as a reciprocal value.
///
/// This type represents the sample rate as a reciprocal (1/sample_rate) for efficient
/// time calculations in audio processing.
pub type Samplerate = Recip<f64>;

/// A slice of parameter events.
///
/// This type represents a time-stamped collection of parameter value changes,
/// allowing modules to process parameter automation.
pub type ParamSlice = EventSlice<f32>;

/// A slice of MIDI note events.
///
/// This type represents a time-stamped collection of MIDI note events,
/// allowing modules to process note on/off and other MIDI events.
pub type NoteSlice = EventSlice<note::NoteEvent>;

/// Result of preparing a module for processing.
///
/// This struct is returned by the `prepare` method of the `Module` trait and
/// contains information about the module's state after preparation.
#[derive(Debug, Copy, Clone)]
pub struct PrepareResult {
    /// The latency introduced by the module in seconds.
    ///
    /// This value represents the delay between input and output caused by the module's
    /// processing, which can be used for latency compensation in the host.
    pub latency: f64,
}

/// Result of processing a block of audio.
///
/// This struct is returned by the `process` method of the `Module` trait and
/// contains information about the module's state after processing.
#[derive(Debug, Copy, Clone)]
pub struct ProcessResult {
    /// The number of additional blocks the module needs to output its tail.
    ///
    /// This is used for modules that produce output even after their input has stopped,
    /// such as reverbs and delays. If `None`, the module has no tail.
    pub tail: Option<NonZeroU32>,
}

/// Core trait for audio processing modules.
///
/// This trait defines the interface for all audio processing modules in the `clogbox`
/// framework. It provides methods for preparing the module for processing and
/// processing blocks of audio samples.
///
/// Modules can have audio inputs and outputs, parameter inputs and outputs, and
/// note (MIDI) inputs and outputs, all of which are defined by associated types.
pub trait Module {
    /// The sample type used by this module (typically f32 or f64).
    type Sample;

    /// The enum type defining audio input channels.
    type AudioIn: Enum;

    /// The enum type defining audio output channels.
    type AudioOut: Enum;

    /// The enum type defining parameter input types.
    type ParamsIn: Enum;

    /// The enum type defining parameter output types.
    type ParamsOut: Enum;

    /// The enum type defining note input types.
    type NoteIn: Enum;

    /// The enum type defining note output types.
    type NoteOut: Enum;

    /// Prepares the module for processing at the given sample rate and block size.
    ///
    /// This method is called before processing begins or when processing parameters
    /// change, allowing the module to initialize its internal state.
    ///
    /// # Parameters
    ///
    /// * `sample_rate` - The sample rate at which the module will operate
    /// * `block_size` - The maximum number of samples per processing block
    ///
    /// # Returns
    ///
    /// A `PrepareResult` containing information about the module's state after preparation.
    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult;

    /// Processes a block of audio samples.
    ///
    /// This method is called for each block of audio to be processed. It receives
    /// a context containing input audio, parameters, and notes, and produces
    /// output audio, parameters, and notes.
    ///
    /// # Parameters
    ///
    /// * `context` - The processing context containing inputs and outputs
    ///
    /// # Returns
    ///
    /// A `ProcessResult` containing information about the module's state after processing.
    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult;
}
