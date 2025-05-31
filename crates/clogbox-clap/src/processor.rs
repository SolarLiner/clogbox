use crate::main_thread::{Layout, MainThread, Plugin};
#[cfg(feature = "gui")]
use crate::params::{create_notifier_listener, ParamChangeEvent, ParamChangeKind, ParamListener};
use crate::params::{ParamId, ParamIdExt};
use crate::shared::Shared;
use clack_extensions::params::PluginAudioProcessorParams;
use clack_plugin::events::event_types::{NoteChokeEvent, NoteOffEvent, NoteOnEvent, ParamValueEvent};
#[cfg(feature = "gui")]
use clack_plugin::events::event_types::{ParamGestureBeginEvent, ParamGestureEndEvent};
use clack_plugin::events::Match;
use clack_plugin::host::HostAudioProcessorHandle;
pub use clack_plugin::host::HostSharedHandle;
pub use clack_plugin::plugin::PluginError;
use clack_plugin::prelude::*;
pub use clack_plugin::process::{PluginAudioConfiguration, ProcessStatus};
#[cfg(feature = "gui")]
use clack_plugin::utils::Cookie;
use clogbox_enum::enum_map::{EnumMap, EnumMapArray, EnumMapRef};
use clogbox_enum::typenum::U16;
use clogbox_enum::{count, enum_iter, Empty, Enum, Sequential};
use clogbox_math::frequency::midi_note_to_frequency;
use clogbox_module::context::{AudioStorage, EventStorage, ProcessContext, StreamContext};
use clogbox_module::eventbuffer::Timestamped;
use clogbox_module::note::{NoteEvent, NoteId};
use clogbox_module::{Module, Samplerate};
use std::marker::PhantomData;
use std::num::{NonZeroU32, Wrapping};
use std::sync::atomic::Ordering;

pub type NoteChannel = Sequential<U16>;

pub struct PluginCreateContext<'a, 'p, P: ?Sized + PluginDsp> {
    pub host: HostSharedHandle<'a>,
    pub processor_main_thread: &'p mut P::Plugin,
    pub params: EnumMapRef<'p, P::ParamsIn, f32>,
    pub audio_config: PluginAudioConfiguration,
}

/// A DSP module that can also be used as the audio processor for a plugin.
///
/// TODO: Note support
pub trait PluginDsp: Send + Module<Sample = f32, ParamsIn: ParamId, ParamsOut = Empty> {
    type Plugin: Plugin<Dsp = Self, Params = Self::ParamsIn>;

    fn create(context: PluginCreateContext<Self>, shared_data: &<Self::Plugin as Plugin>::SharedData) -> Self;
}

pub struct Processor<'a, P: PluginDsp> {
    shared: &'a Shared<P::Plugin>,
    dsp: P,
    audio_in: AudioStorage<P::AudioIn, P::Sample>,
    audio_out: AudioStorage<P::AudioOut, P::Sample>,
    params: EventStorage<P::ParamsIn, f32>,
    note_in: EventStorage<P::NoteIn, NoteEvent>,
    note_out: EventStorage<P::NoteOut, NoteEvent>,
    note_out_map: EnumMapArray<P::NoteOut, Option<(usize, usize)>>,
    note_out_next_id: Wrapping<u32>,
    #[cfg(feature = "gui")]
    params_rx: ParamListener<P::ParamsIn>,
    sample_rate: Samplerate,
    tail: Option<NonZeroU32>,
}

impl<'a, P: 'a + PluginDsp<Plugin: Plugin>> PluginAudioProcessor<'a, Shared<P::Plugin>, MainThread<'a, P::Plugin>>
    for Processor<'a, P>
{
    fn activate(
        host: HostAudioProcessorHandle<'a>,
        main_thread: &mut MainThread<P::Plugin>,
        shared: &'a Shared<P::Plugin>,
        audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        let params = shared.params.read_all_values();
        let sample_rate = Samplerate::new(audio_config.sample_rate);
        let block_size = audio_config.max_frames_count as usize;
        let audio_config = PluginAudioConfiguration {
            min_frames_count: 1,
            ..audio_config
        };
        let context = PluginCreateContext {
            host: host.shared(),
            params: params.to_ref(),
            processor_main_thread: &mut main_thread.plugin,
            audio_config,
        };
        let mut dsp = P::create(context, &shared.user_data);
        let audio_in = AudioStorage::default(block_size);
        let audio_out = AudioStorage::default(block_size);
        let params = EventStorage::with_capacity(512);
        let note_in = EventStorage::with_capacity(512);
        let note_out = EventStorage::with_capacity(512);
        dsp.prepare(sample_rate, audio_config.max_frames_count as _);

        let note_out_map = EnumMapArray::new(|e| {
            P::Plugin::NOTE_OUT_LAYOUT
                .iter()
                .enumerate()
                .find_map(|(port, layout)| {
                    layout
                        .channel_map
                        .iter()
                        .position(|v| *v == e)
                        .map(move |channel| (port, channel))
                })
        });

        shared
            .sample_rate
            .store(sample_rate.value().to_bits(), Ordering::Relaxed);

        #[cfg(feature = "gui")]
        let (tx, rx) = create_notifier_listener(1024);
        #[cfg(feature = "gui")]
        shared.notifier.add_listener(move |event| {
            tx.notify(event.id, event.kind);
        });
        Ok(Self {
            shared,
            dsp,
            audio_in,
            audio_out,
            note_in,
            note_out,
            note_out_map,
            note_out_next_id: Wrapping(0),
            params,
            #[cfg(feature = "gui")]
            params_rx: rx,
            sample_rate,
            tail: None,
        })
    }

    fn process(
        &mut self,
        _process: Process,
        mut audio: Audio,
        mut events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        self.copy_inputs(&audio)?;
        self.copy_events_in(&mut events)?;
        let process_status = self.process_audio(&StreamContext {
            block_size: audio.frames_count() as _,
            sample_rate: self.sample_rate,
        })?;
        self.copy_events_out(&mut events)?;
        self.copy_outputs(&mut audio)?;
        Ok(process_status)
    }
}

#[allow(clippy::needless_range_loop)]
impl<P: PluginDsp> Processor<'_, P> {
    fn copy_inputs(&mut self, audio: &Audio) -> Result<(), PluginError> {
        for (i, port) in audio.input_ports().enumerate() {
            if let Some(channels) = port.channels()?.into_f32() {
                for (j, channel) in channels.iter().enumerate() {
                    let index = P::Plugin::AUDIO_IN_LAYOUT[i].channel_map[j];
                    self.audio_in[index][..channel.len()].copy_from_slice(channel);
                }
            } else if let Some(channels) = port.channels()?.into_f64() {
                for (j, channel) in channels.iter().enumerate() {
                    let index = P::Plugin::AUDIO_IN_LAYOUT[i].channel_map[j];
                    for i in 0..channel.len() {
                        self.audio_in[index][i] = channel[i] as f32;
                    }
                }
            } else {
                return Err(PluginError::Message("Unsupported input channel type"));
            }
        }
        Ok(())
    }

    fn copy_events_in(&mut self, events: &mut Events) -> Result<(), PluginError> {
        for buf in self.params.values_mut() {
            buf.clear();
        }
        for buf in self.note_in.values_mut() {
            buf.clear();
        }
        for event in events.input.iter() {
            if let Some(ev) = event.as_event::<ParamValueEvent>() {
                self.insert_param_value(ev.param_id(), ev.time() as _, ev.value());
            } else if let Some(ev) = event.as_event::<NoteOnEvent>() {
                self.insert_note_on(ev);
            } else if let Some(ev) = event.as_event::<NoteOffEvent>() {
                self.insert_note_off(ev);
            } else if let Some(ev) = event.as_event::<NoteChokeEvent>() {
                self.insert_note_choke(ev);
            }
        }

        // Retrieve (and publish to host) params received by the GUI
        #[cfg(feature = "gui")]
        for event in &mut self.params_rx {
            let clap_id = ClapId::new(event.id.to_usize() as _);
            let result = match event.kind {
                ParamChangeKind::GestureBegin => events.output.try_push(ParamGestureBeginEvent::new(0, clap_id)),
                ParamChangeKind::GestureEnd => events.output.try_push(ParamGestureEndEvent::new(0, clap_id)),
                ParamChangeKind::ValueChange(v) => {
                    (*self.params)[event.id].push(0, v);
                    events.output.try_push(ParamValueEvent::new(
                        0,
                        clap_id,
                        Pckn::match_all(),
                        event.id.denormalized_to_clap_value(v),
                        Cookie::empty(),
                    ))
                }
            };
            if let Err(err) = result {
                log::debug!("Failed to push event: {}", err);
            }
        }

        // Send last param values to the shared state
        for param in enum_iter::<P::ParamsIn>() {
            let Some(&Timestamped { data: value, .. }) = self.params[param].last() else {
                continue;
            };
            self.shared.params.set(param, value);
            // self.shared.notifier.notify(ParamChangeEvent {
            //     id: param,
            //     kind: ParamChangeKind::ValueChange(value),
            // });
        }
        Ok(())
    }

    fn copy_events_out(&mut self, events: &mut Events) -> Result<(), PluginError> {
        for out in enum_iter::<P::NoteOut>() {
            let Some((port, channel)) = self.note_out_map[out] else {
                self.note_out[out].clear();
                continue;
            };
            let mut next_id = self.note_out_next_id;
            for &Timestamped { data, timestamp } in self.note_out[out].as_raw_slice() {
                let note_id = next_id.0;
                next_id += 1;
                match data {
                    NoteEvent::NoteOn { id, velocity, .. } => {
                        events.output.try_push(NoteOnEvent::new(
                            timestamp as _,
                            Pckn::new(port as u16, channel as u16, id.number as u16, note_id),
                            velocity as _,
                        ))?;
                    }
                    NoteEvent::NoteOff { id, velocity, .. } => {
                        events.output.try_push(NoteOffEvent::new(
                            timestamp as _,
                            Pckn::new(port as u16, channel as u16, id.number as u16, note_id),
                            velocity as _,
                        ))?;
                    }
                    NoteEvent::Choke { id } => {
                        events.output.try_push(NoteChokeEvent::new(
                            timestamp as _,
                            Pckn::new(port as u16, channel as u16, id.number as u16, note_id),
                        ))?;
                    }
                }
            }
            self.note_out_next_id = next_id;
            self.note_out[out].clear();
        }
        Ok(())
    }

    fn insert_param_value(&mut self, clap_id: Option<ClapId>, time: usize, value: f64) -> bool {
        let Some(param) = clap_id.and_then(|id| {
            let index = id.get() as usize;
            if index < count::<P::ParamsIn>() {
                Some(P::ParamsIn::from_usize(index))
            } else {
                None
            }
        }) else {
            return true;
        };
        (*self.params)[param].push(time, param.clap_value_to_denormalized(value));
        false
    }

    fn insert_note_on(&mut self, ev: &NoteOnEvent) {
        for (channel, note_in) in Self::target_note_in(ev.port_index(), ev.channel()) {
            let note_id = NoteId {
                channel,
                number: ev.key().into_specific().expect("Note key cannot be 'All'") as _,
            };
            (*self.note_in)[note_in].push(
                ev.time() as _,
                NoteEvent::NoteOn {
                    id: note_id,
                    velocity: ev.velocity() as _,
                    frequency: midi_note_to_frequency(note_id.number) as _,
                },
            );
        }
    }

    fn insert_note_off(&mut self, ev: &NoteOffEvent) {
        for (channel, note) in Self::target_note_in(ev.port_index(), ev.channel()) {
            let note_id = NoteId {
                channel,
                number: ev.key().into_specific().expect("Note key cannot be 'All'") as _,
            };
            (*self.note_in)[note].push(
                ev.time() as _,
                NoteEvent::NoteOff {
                    id: note_id,
                    velocity: ev.velocity() as _,
                    frequency: midi_note_to_frequency(note_id.number) as _,
                },
            );
        }
    }

    fn insert_note_choke(&mut self, ev: &NoteChokeEvent) {
        for (channel, note) in Self::target_note_in(ev.port_index(), ev.channel()) {
            let note_id = NoteId {
                channel,
                number: ev.key().into_specific().expect("Note key cannot be 'All'") as _,
            };
            (*self.note_in)[note].push(ev.time() as _, NoteEvent::Choke { id: note_id });
        }
    }

    fn target_note_in(port: Match<u16>, channel: Match<u16>) -> impl Iterator<Item = (u8, P::NoteIn)> {
        iter_match_u16(port, P::Plugin::NOTE_IN_LAYOUT.len() as _).flat_map(move |port| {
            let port = P::Plugin::NOTE_IN_LAYOUT[port as usize];
            iter_match_u16(channel, port.channel_map.len() as _)
                .map(move |channel| (channel as _, port.channel_map[channel as usize]))
        })
    }

    fn copy_outputs(&mut self, audio: &mut Audio) -> Result<(), PluginError> {
        for (i, mut port) in audio.output_ports().enumerate() {
            if let Some(mut channels) = port.channels()?.into_f32() {
                for (j, channel) in channels.iter_mut().enumerate() {
                    let index = P::Plugin::AUDIO_OUT_LAYOUT[i].channel_map[j];
                    let slice = &mut self.audio_out[index][..channel.len()];
                    for x in &mut *slice {
                        if !x.is_finite() {
                            *x = 0.0;
                        }
                    }
                    channel.copy_from_slice(slice);
                }
            } else if let Some(mut channels) = port.channels()?.into_f64() {
                for (j, channel) in channels.iter_mut().enumerate() {
                    let index = P::Plugin::AUDIO_OUT_LAYOUT[i].channel_map[j];
                    for i in 0..channel.len() {
                        let y = self.audio_out[index][i] as f64;
                        channel[i] = if y.is_finite() { y } else { 0.0 };
                    }
                }
            }
        }
        Ok(())
    }

    fn process_audio(&mut self, stream_context: &StreamContext) -> Result<ProcessStatus, PluginError> {
        let ctx = ProcessContext {
            audio_in: &self.audio_in,
            audio_out: &mut self.audio_out,
            params_in: &self.params,
            params_out: &mut EventStorage::empty(),
            note_in: &self.note_in,
            note_out: &mut self.note_out,
            stream_context,
            __phantom: PhantomData,
        };
        let result = self.dsp.process(ctx);
        self.tail = result.tail;
        if self.tail.is_some() {
            Ok(ProcessStatus::Tail)
        } else {
            Ok(ProcessStatus::ContinueIfNotQuiet)
        }
    }
}

fn iter_match_u16(m: Match<u16>, num_values: u16) -> impl Iterator<Item = u16> {
    enum MatchIter {
        Specific { value: u16, done: bool },
        All { next: u16, max: u16 },
    }
    impl Iterator for MatchIter {
        type Item = u16;

        fn next(&mut self) -> Option<Self::Item> {
            match self {
                Self::Specific { value, done } if !*done => {
                    *done = true;
                    Some(*value)
                }
                Self::All { next, max } if *next <= *max => {
                    let ret = *next;
                    *next += 1;
                    Some(ret)
                }
                _ => None,
            }
        }
    }

    match m {
        Match::All => MatchIter::All {
            next: 0,
            max: num_values - 1,
        },
        Match::Specific(value) => MatchIter::Specific { value, done: false },
    }
}

impl<P: PluginDsp> PluginAudioProcessorParams for Processor<'_, P> {
    fn flush(&mut self, input_parameter_changes: &InputEvents, _: &mut OutputEvents) {
        for event in input_parameter_changes {
            let Some(ev) = event.as_event::<ParamValueEvent>() else {
                continue;
            };
            let Some(param) = ev.param_id().and_then(|id| {
                let index = id.get() as usize;
                if index < count::<P::ParamsIn>() {
                    Some(P::ParamsIn::from_usize(index))
                } else {
                    None
                }
            }) else {
                continue;
            };
            self.shared.params.set_normalized(param, ev.value() as _);
        }
    }
}
