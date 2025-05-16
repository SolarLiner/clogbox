use crate::main_thread::{MainThread, Plugin};
#[cfg(feature = "gui")]
use crate::params::ParamListener;
use crate::params::{ParamChangeKind, ParamId, ParamIdExt};
use crate::shared::Shared;
use clack_extensions::params::PluginAudioProcessorParams;
use clack_plugin::events::event_types::{ParamGestureBeginEvent, ParamGestureEndEvent, ParamValueEvent};
use clack_plugin::host::HostAudioProcessorHandle;
pub use clack_plugin::host::HostSharedHandle;
pub use clack_plugin::plugin::PluginError;
use clack_plugin::prelude::*;
pub use clack_plugin::process::{PluginAudioConfiguration, ProcessStatus};
use clack_plugin::utils::Cookie;
use clogbox_enum::enum_map::EnumMapRef;
use clogbox_enum::{count, enum_iter, Empty, Enum};
use clogbox_module::context::{AudioStorage, EventStorage, ProcessContext, StreamContext};
use clogbox_module::eventbuffer::Timestamped;
use clogbox_module::{Module, Samplerate};
#[cfg(feature = "gui")]
use ringbuf::traits::Consumer;
use std::marker::PhantomData;
use std::num::NonZeroU32;

pub struct PluginCreateContext<'a, 'p, P: ?Sized + PluginDsp> {
    pub host: HostSharedHandle<'a>,
    pub processor_main_thread: &'p mut P::Plugin,
    pub params: EnumMapRef<'p, P::ParamsIn, f32>,
    pub audio_config: PluginAudioConfiguration,
}

/// A DSP module that can also be used as the audio processor for a plugin.
///
/// TODO: Note support
pub trait PluginDsp:
    Send + Module<Sample = f32, ParamsIn: ParamId, ParamsOut = Empty, NoteIn = Empty, NoteOut = Empty>
{
    type Plugin: Plugin<Dsp = Self, Params = Self::ParamsIn>;

    fn create(context: PluginCreateContext<Self>, shared_data: &<Self::Plugin as Plugin>::SharedData) -> Self;
}

pub struct Processor<'a, P: PluginDsp> {
    shared: &'a Shared<P::Plugin>,
    dsp: P,
    audio_in: AudioStorage<P::AudioIn, P::Sample>,
    audio_out: AudioStorage<P::AudioOut, P::Sample>,
    params: EventStorage<P::ParamsIn, f32>,
    sample_rate: Samplerate,
    tail: Option<NonZeroU32>,
    #[cfg(feature = "gui")]
    dsp_listener: ParamListener<P::ParamsIn>,
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
        let audio_in = AudioStorage::default(audio_config.max_frames_count as usize);
        let audio_out = AudioStorage::default(audio_config.max_frames_count as usize);
        let params = EventStorage::with_capacity(512);
        #[cfg(feature = "gui")]
        let dsp_listener = main_thread.dsp_listener.take().unwrap();
        dsp.prepare(
            Samplerate::new(audio_config.sample_rate),
            audio_config.max_frames_count as _,
        );
        Ok(Self {
            shared,
            dsp,
            audio_in,
            audio_out,
            params,
            #[cfg(feature = "gui")]
            dsp_listener,
            sample_rate: Samplerate::new(audio_config.sample_rate),
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
        self.copy_events(&mut events)?;
        let process_status = self.process_audio(&StreamContext {
            block_size: audio.frames_count() as _,
            sample_rate: self.sample_rate,
        })?;
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
                    let index = P::Plugin::INPUT_LAYOUT[i].channel_map[j];
                    self.audio_in[index][..channel.len()].copy_from_slice(channel);
                }
            } else if let Some(channels) = port.channels()?.into_f64() {
                for (j, channel) in channels.iter().enumerate() {
                    let index = P::Plugin::INPUT_LAYOUT[i].channel_map[j];
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

    fn copy_events(&mut self, events: &mut Events) -> Result<(), PluginError> {
        for buf in self.params.values_mut() {
            buf.clear();
        }
        for event in events.input.iter() {
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
            (*self.params)[param].push(ev.time() as _, param.clap_value_to_denormalized(ev.value()));
        }

        // Retrieve (and publish to host) params received by the GUI
        #[cfg(feature = "gui")]
        for event in &mut self.dsp_listener {
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
                        event.id.mapping().normalize(v) as _,
                        Cookie::empty(),
                    ))
                }
            };
            if let Err(err) = result {
                eprintln!("Failed to push event: {}", err);
            }
        }

        // Send last param values to the shared state
        for param in enum_iter::<P::ParamsIn>() {
            let Some(&Timestamped { data: value, .. }) = self.params[param].last() else {
                continue;
            };
            self.shared.params.set(param, value);
        }
        Ok(())
    }

    fn copy_outputs(&mut self, audio: &mut Audio) -> Result<(), PluginError> {
        for (i, mut port) in audio.output_ports().enumerate() {
            if let Some(mut channels) = port.channels()?.into_f32() {
                for (j, channel) in channels.iter_mut().enumerate() {
                    let index = P::Plugin::OUTPUT_LAYOUT[i].channel_map[j];
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
                    let index = P::Plugin::OUTPUT_LAYOUT[i].channel_map[j];
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
            note_in: &EventStorage::empty(),
            note_out: &mut EventStorage::empty(),
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
