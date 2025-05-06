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
