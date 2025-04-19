#[macro_export]
macro_rules! module_wrapper {
    ($output:ident: $base:ty) => {
pub struct $output(pub $base);

impl Module for $output {
    type Sample = <$base as Module>::Sample;
    type AudioIn = <$base as Module>::AudioIn;
    type AudioOut = <$base as Module>::AudioOut;
    type ParamsIn = <$base as Module>::ParamsIn;
    type ParamsOut = <$base as Module>::ParamsOut;
    type NoteIn = <$base as Module>::NoteIn;
    type NoteOut = <$base as Module>::NoteOut;

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
        self.0.prepare(sample_rate, block_size)
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        let context: ProcessContext<$base> = ProcessContext {
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

impl<$($targ: $tbound),*> Module for $output<$($targ),*> {
    type Sample = <$base as Module>::Sample;
    type AudioIn = <$base as Module>::AudioIn;
    type AudioOut = <$base as Module>::AudioOut;
    type ParamsIn = <$base as Module>::ParamsIn;
    type ParamsOut = <$base as Module>::ParamsOut;
    type NoteIn = <$base as Module>::NoteIn;
    type NoteOut = <$base as Module>::NoteOut;

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
        self.0.prepare(sample_rate, block_size)
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        let context: ProcessContext<$base> = ProcessContext {
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