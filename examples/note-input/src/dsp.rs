use clogbox_clap::{Plugin, PluginCreateContext, PluginDsp};
use clogbox_enum::typenum::{U1, U63};
use clogbox_enum::{seq, Empty, Mono, Sequential};
use clogbox_math::interpolation::Sinc;
use clogbox_module::context::{ProcessContext, UnifiedEvent};
use clogbox_module::note::{NoteEvent, NoteId};
use clogbox_module::{Module, PrepareResult, ProcessResult, Samplerate};
use clogbox_oscillators::Wavetable;

#[derive(Debug, Clone, Copy)]
struct Note {
    id: NoteId,
    frequency: f32,
    velo_sqrt: f32,
}

pub(crate) struct Dsp {
    wavetable: Wavetable<f32, Sinc<U63>>,
    cur_note: Option<Note>,
}

impl PluginDsp for Dsp {
    type Plugin = super::NoteInput;

    fn create(context: PluginCreateContext<Self>, _: &<Self::Plugin as Plugin>::SharedData) -> Self {
        let sample_rate = context.audio_config.sample_rate as f32;
        Self {
            wavetable: Wavetable::new(sample_rate, 440.0, Default::default(), {
                const WT_SIZE: usize = 512;
                let step = (WT_SIZE as f32).recip();
                (0..WT_SIZE)
                    .map(move |i| step * i as f32)
                    .map(|phase| 2.0 * phase - 1.0)
            }),
            cur_note: None,
        }
    }
}

impl Module for Dsp {
    type Sample = f32;
    type AudioIn = Empty;
    type AudioOut = Mono;
    type ParamsIn = Empty;
    type ParamsOut = Empty;
    type NoteIn = Sequential<U1>;
    type NoteOut = Sequential<U1>;

    fn prepare(&mut self, sample_rate: Samplerate, block_size: usize) -> PrepareResult {
        self.wavetable.prepare(sample_rate, block_size)
    }

    fn process(&mut self, context: ProcessContext<Self>) -> ProcessResult {
        for (range, events) in context
            .events_in
            .slice()
            .chunk_events(context.stream_context.block_size)
        {
            for event in events {
                match event.data {
                    UnifiedEvent::Parameter(..) => unreachable!(),
                    UnifiedEvent::Note(_, event) => {
                        context
                            .events_out
                            .push(range.start, UnifiedEvent::Note(seq(0), event.clone()));
                        match event {
                            NoteEvent::NoteOn {
                                id,
                                frequency,
                                velocity,
                            } => {
                                self.cur_note.replace(Note {
                                    id,
                                    frequency,
                                    velo_sqrt: velocity.sqrt(),
                                });
                            }
                            NoteEvent::NoteOff { id, .. } | NoteEvent::Choke { id }
                                if Some(id) == self.cur_note.as_ref().map(|n| n.id) =>
                            {
                                self.cur_note.take();
                            }
                            _ => {}
                        }
                    }
                }
            }
            self.process_slice(&mut context.audio_out[Mono][range]);
        }
        ProcessResult { tail: None }
    }
}

impl Dsp {
    fn process_slice(&mut self, slice: &mut [f32]) {
        let Some(note) = self.cur_note else {
            slice.fill(0.0);
            return;
        };
        self.wavetable.set_frequency(note.frequency);
        self.wavetable.process_slice(slice);
        for s in slice {
            *s *= note.velo_sqrt;
        }
    }
}
