use crate::graph::context::{GraphContext, GraphContextImpl, RawGraphContext};
use crate::graph::slots::Slots;
use crate::graph::SlotType;
use clogbox_enum::Enum;
use std::error::Error;
use thiserror::Error;

/// Represents the metadata and configuration for a stream of audio data.
#[derive(Debug, Copy, Clone)]
pub struct StreamData {
    /// The sample rate of the audio stream, in samples per second.
    pub sample_rate: f64,
    /// The beats per minute (BPM) of the audio stream.
    pub bpm: f64,
    /// The size of a processing block in samples.
    pub block_size: usize,
}

impl StreamData {
    /// Calculates the time duration of one sample in seconds.
    ///
    /// # Returns
    ///
    /// The time duration of one sample as a [`f64`] value.
    /// Calculates the time duration of one sample in seconds.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::graph::module::StreamData;
    /// let stream_data = StreamData {
    ///     sample_rate: 44100.0,
    ///     bpm: 120.0,
    ///     block_size: 512,
    /// };
    /// let time_duration = stream_data.dt();

    /// assert_eq!(1./44100., time_duration);
    /// ```
    pub fn dt(&self) -> f64 {
        self.sample_rate.recip()
    }

    /// Calculates the length of a given number of beats in minutes.
    ///
    /// # Arguments
    ///
    /// * `beats` - The number of beats to calculate the length for.
    ///
    /// # Returns
    ///
    /// The length of the specified number of beats in minutes as a [`f64`] value.
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::graph::module::StreamData;
    /// let stream_data = StreamData {
    ///     sample_rate: 44100.0,
    ///     bpm: 120.0,
    ///     block_size: 512,
    /// };
    /// let beats = 4.0;
    /// let length = stream_data.beat_length(beats);
    /// assert_eq!(2.0, length);
    /// ```
    pub fn beat_length(&self, beats: f64) -> f64 {
        beats * 60. / self.bpm
    }

    /// Calculates the length of a given number of beats in samples.
    ///
    /// # Arguments
    /// * `beats` - The number of beats to calculate the length for.
    ///
    /// # Returns
    /// The length of the specified number of beats in samples as a [`f64`] value.
    pub fn beat_sample_length(&self, beats: f64) -> f64 {
        self.sample_rate * self.beat_length(beats)
    }
}

/// Represents the status of a process.
#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub enum ProcessStatus {
    /// The process is currently running.
    #[default]
    Running,
    /// The process will begin returning silence after this many samples, provided the input is also
    /// silent.
    Tail(u64),
    /// The process has not processed anything.
    Silent,
}

/// Represents potential errors that can occur when working with a module.
#[derive(Debug, Error)]
pub enum ModuleError {
    /// Indicates that a required input is missing.
    ///
    /// # Fields
    ///
    /// * `0` - The [`SlotType`] of the required input.
    /// * `1` - A [`String`] description of the input.
    ///
    /// This error is returned when the processing of the module cannot continue due to
    /// a missing input slot.
    #[error("Missing required input: {1} of type {0:?}")]
    MissingRequiredInput(SlotType, String),

    /// Indicates that a required output is missing.
    ///
    /// # Fields
    ///
    /// * `0` - The [`SlotType`] of the required output.
    /// * `1` - A [`String`] description of the output.
    ///
    /// This error is returned when the module is unable to provide required output data.
    #[error("Missing required output: {1} of type {0:?}")]
    MissingRequiredOutput(SlotType, String),

    /// Indicates that the module is not ready to process data.
    ///
    /// This may occur if the module requires an initialization process or some external
    /// dependencies that have not been fulfilled yet.
    #[error("Module not ready")]
    NotReady,

    /// Indicates a fatal error that occurred during module operation.
    ///
    /// # Fields
    ///
    /// * `0` - The inner error wrapped in a `Box<dyn Error>`.
    ///
    /// This error is returned when a critical issue occurs, preventing the module from
    /// functioning properly.
    #[error("Fatal error: {0}")]
    Fatal(#[from] Box<dyn Error>),
}

/// Represents a module that can be processed in the graph context.
///
/// This trait allows defining a module with specific inputs and outputs, as well as a process
/// function that operates on the associated [`GraphContext`].
///
/// # Associated Types
///
/// - `Sample`: The type representing an individual sample of data handled by the module (e.g.,
/// [`f32`] for audio data).
/// - `Inputs`: The type representing the inputs of the module, which must implement the [`Slots`]
/// trait.
/// - `Outputs`: The type representing the outputs of the module, which must implement the
/// [`Slots`] trait.
///
/// # Example
///
/// ```
/// use clogbox_core::graph::module::{Module, ModuleError, ProcessStatus, StreamData};
/// use clogbox_core::graph::context::GraphContext;
/// use clogbox_core::graph::SlotType;
///
/// struct ExampleModule;
///
/// impl Module for ExampleModule {
///     type Sample = f32;
///     type Inputs = (); // Define input slots
///     type Outputs = (); // Define output slots
///
///     fn process(
///         &mut self,
///         graph_context: GraphContext<Self>,
///     ) -> Result<ProcessStatus, ModuleError> {
///         // Implement the module's processing logic here.
///         Ok(ProcessStatus::Running)
///     }
/// }
/// ```
pub trait Module {
    /// Type representing an individual sample of data (e.g., [`f32`] for audio, [`i16`], etc.).
    type Sample;

    /// Type representing the input slots of the module. This type must implement the [`Slots`]
    /// trait.
    type Inputs: Slots;

    /// Type representing the output slots of the module. This type must implement the [`Slots`]
    /// trait.
    type Outputs: Slots;

    /// Processes the module in the context of the graph.
    ///
    /// # Arguments
    ///
    /// - `graph_context`: A [`GraphContext`] providing information for processing inputs, outputs,
    ///   stream data, and other utility methods.
    ///
    /// # Returns
    ///
    /// - `Ok(ProcessStatus)` if processing succeeds, where [`ProcessStatus`] indicates the current
    ///   processing state of the module (e.g., [`Running`](ProcessStatus::Running), [`Tail`]
    /// (ProcessStatus::Tail), or [`Silent`](ProcessStatus::Silent).
    /// - `Err(ModuleError)` if an error occurs during processing (e.g., missing inputs or fatal errors).
    ///
    /// # Example
    ///
    /// ```
    /// use clogbox_core::graph::module::{Module, ModuleError, ProcessStatus, StreamData};
    /// use clogbox_core::graph::context::GraphContext;
    ///
    /// struct MyModule;
    ///
    /// impl Module for MyModule {
    ///     type Sample = f32;
    ///     type Inputs = (); // Define input slots
    ///     type Outputs = (); // Define output slots
    ///
    ///     fn process(
    ///         &mut self,
    ///         graph_context: GraphContext<Self>,
    ///     ) -> Result<ProcessStatus, ModuleError> {
    ///         // Your processing logic here
    ///         Ok(ProcessStatus::Running)
    ///     }
    /// }
    /// ```
    fn process(&mut self, graph_context: GraphContext<Self>) -> Result<ProcessStatus, ModuleError>;
}

pub trait RawModule {
    type Sample;

    fn process(
        &mut self,
        graph_context: RawGraphContext<Self::Sample>,
    ) -> Result<ProcessStatus, ModuleError>;
}

impl<M: Module> RawModule for M {
    type Sample = M::Sample;

    fn process(
        &mut self,
        graph_context: RawGraphContext<Self::Sample>,
    ) -> Result<ProcessStatus, ModuleError> {
        let input_index =
            |id: M::Inputs| (graph_context.input_index)(id.slot_type(), id.to_usize());
        let output_index =
            |id: M::Outputs| (graph_context.output_index)(id.slot_type(), id.to_usize());
        Module::process(
            self,
            GraphContextImpl {
                stream_data: graph_context.stream_data,
                input_index: &input_index,
                output_index: &output_index,
                storage: graph_context.storage,
            },
        )
    }
}
