//! # Macros for simplifying module implementation
//!
//! This module contains macros that reduce boilerplate when implementing modules
//! and related types.

/// The `module_wrapper` macro is used to simplify the process of creating a wrapper type around a base type that
/// implements the [`Module`](crate::Module) trait. This wrapper forwards all trait methods to the inner type while
/// maintaining compatibility with the [`Module`](crate::Module) interface.
///
/// This allows creating "newtype"-like types that can then take on new implementation, circumventing Rust's orphan
/// rule.
///
/// The macro supports both non-generic and generic base types.
///
/// # Macro Syntax
///
/// ## Non-Generic Base
///
/// ```ignore
/// module_wrapper!(WrapperName: BaseType);
/// ```
/// * `WrapperName` is the name of the wrapper struct to be generated.
/// * `BaseType` is the existing type that implements the `Module` trait and is being wrapped.
///
/// ## Generic Base
///
/// ```ignore
/// module_wrapper!(WrapperName<T1: Bound1, T2: Bound2>: BaseType<T1, T2>);
/// ```
/// * `WrapperName` is the name of the wrapper struct to be generated.
/// * `T1: Bound1, T2: Bound2` defines generic type parameters and their bounds/constraints.
/// * `BaseType<T1, T2>` is the generic base type being wrapped.
///
/// # Code Generation
///
/// For a given input, the macro generates:
/// 1. A wrapper struct that stores an instance of the base type.
/// 2. An `impl` block for the `Module` trait, where all method calls are forwarded to the base type.
#[macro_export]
macro_rules! module_wrapper {
    ($output:ident: $base:ty) => {
pub struct $output(pub $base);

impl $crate::Module for $output {
    type Sample = <$base as $crate::Module>::Sample;
    type AudioIn = <$base as $crate::Module>::AudioIn;
    type AudioOut = <$base as $crate::Module>::AudioOut;
    type ParamsIn = <$base as $crate::Module>::ParamsIn;
    type ParamsOut = <$base as $crate::Module>::ParamsOut;
    type NoteIn = <$base as $crate::Module>::NoteIn;
    type NoteOut = <$base as $crate::Module>::NoteOut;

    fn prepare(&mut self, sample_rate: $crate::Samplerate, block_size: usize) -> $crate::PrepareResult {
        self.0.prepare(sample_rate, block_size)
    }

    fn process(&mut self, context: $crate::context::ProcessContext<Self>) -> $crate::ProcessResult {
        let context = $crate::context::ProcessContext {
            audio_in: context.audio_in,
            audio_out: context.audio_out,
            params_in: context.params_in,
            params_out: context.params_out,
            note_in: context.note_in,
            note_out: context.note_out,
            stream_context: context.stream_context,
            __phantom: Default::default(),
        };
        self.0.process(context)
    }
}
    };

    ($output:ident<$($targ:ident: $tbound:tt),+>: $base:ty) => {
pub struct $output<$($targ: $tbound),*>(pub $base);

impl<$($targ: $tbound),*> $crate::Module for $output<$($targ),*> {
    type Sample = <$base as $crate::Module>::Sample;
    type AudioIn = <$base as $crate::Module>::AudioIn;
    type AudioOut = <$base as $crate::Module>::AudioOut;
    type ParamsIn = <$base as $crate::Module>::ParamsIn;
    type ParamsOut = <$base as $crate::Module>::ParamsOut;
    type NoteIn = <$base as $crate::Module>::NoteIn;
    type NoteOut = <$base as $crate::Module>::NoteOut;

    fn prepare(&mut self, sample_rate: $crate::Samplerate, block_size: usize) -> $crate::PrepareResult {
        self.0.prepare(sample_rate, block_size)
    }

    fn process(&mut self, context: $crate::context::ProcessContext<Self>) -> $crate::ProcessResult {
        let context = $crate::context::ProcessContext {
            audio_in: context.audio_in,
            audio_out: context.audio_out,
            params_in: context.params_in,
            params_out: context.params_out,
            note_in: context.note_in,
            note_out: context.note_out,
            stream_context: context.stream_context,
            __phantom: Default::default(),
        };
        self.0.process(context)
    }
}
    };
}
