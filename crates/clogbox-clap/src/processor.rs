use crate::main_thread::{MainThread, Plugin};
use crate::params::{ParamId, ParamStorage};
use crate::shared::Shared;
use clack_extensions::params::PluginAudioProcessorParams;
use clack_plugin::events::event_types::ParamValueEvent;
use clack_plugin::events::io::InputEventsIter;
use clack_plugin::host::HostAudioProcessorHandle;
pub use clack_plugin::host::HostSharedHandle;
pub use clack_plugin::plugin::PluginError;
use clack_plugin::prelude::*;
pub use clack_plugin::process::{PluginAudioConfiguration, ProcessStatus};
use clogbox_enum::enum_map::{EnumMapArray, EnumMapMut, EnumMapRef};
use clogbox_enum::{count, Enum};
use std::ops;

pub struct PluginCreateContext<'a, 'p, P: ?Sized + PluginDsp> {
    pub host: HostSharedHandle<'a>,
    pub processor_main_thread: &'p mut P::Plugin,
    pub params: EnumMapRef<'p, P::Params, f32>,
    pub audio_config: PluginAudioConfiguration,
}

pub trait PluginDsp: Send {
    type Plugin: Plugin<Dsp = Self, Params = Self::Params>;
    type Params: ParamId;
    type Inputs: Enum;
    type Outputs: Enum;

    fn create(context: PluginCreateContext<Self>) -> Self;

    fn set_param(&mut self, id: Self::Params, value: f32);

    fn process<In: ops::Deref<Target = [f32]>, Out: ops::DerefMut<Target = [f32]>>(
        &mut self,
        frame_count: usize,
        inputs: EnumMapRef<Self::Inputs, In>,
        outputs: EnumMapMut<Self::Outputs, Out>,
    ) -> Result<ProcessStatus, PluginError>;
}

pub struct Processor<P: PluginDsp> {
    params: ParamStorage<P::Params>,
    dsp: P,
    inputs: EnumMapArray<P::Inputs, Box<[f32]>>,
    outputs: EnumMapArray<P::Outputs, Box<[f32]>>,
}

impl<'a, P: 'a + PluginDsp> PluginAudioProcessor<'a, Shared<P::Params>, MainThread<P::Plugin>> for Processor<P> {
    fn activate(
        host: HostAudioProcessorHandle<'a>,
        main_thread: &mut MainThread<P::Plugin>,
        shared: &'a Shared<P::Params>,
        audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        let params = shared.params.read_all_values();
        let inputs =
            EnumMapArray::new(|_| Box::from_iter(std::iter::repeat_n(0.0, audio_config.max_frames_count as usize)));
        let outputs =
            EnumMapArray::new(|_| Box::from_iter(std::iter::repeat_n(0.0, audio_config.max_frames_count as usize)));
        let audio_config = PluginAudioConfiguration {
            min_frames_count: 1,
            ..audio_config
        };
        let context = PluginCreateContext {
            host: host.shared(),
            params: params.to_ref(),
            processor_main_thread: &mut main_thread.processor_main_thread,
            audio_config,
        };
        let dsp = P::create(context);
        Ok(Self {
            params: shared.params.clone(),
            dsp,
            inputs,
            outputs,
        })
    }

    fn process(&mut self, _process: Process, mut audio: Audio, events: Events) -> Result<ProcessStatus, PluginError> {
        self.copy_inputs(&audio)?;
        let mut status = ProcessStatus::ContinueIfNotQuiet;
        for batch in events.input.batch() {
            self.process_events(batch.events());
            status = status.combined_with(self.process_audio(
                batch.first_sample()..batch.next_batch_first_sample().unwrap_or(audio.frames_count() as _),
            )?);
        }
        self.copy_outputs(&mut audio)?;
        Ok(status)
    }
}

#[allow(clippy::needless_range_loop)]
impl<P: PluginDsp> Processor<P> {
    fn copy_inputs(&mut self, audio: &Audio) -> Result<(), PluginError> {
        for (i, port) in audio.input_ports().enumerate() {
            if let Some(channels) = port.channels()?.into_f32() {
                for (j, channel) in channels.iter().enumerate() {
                    let index = P::Plugin::INPUT_LAYOUT[i].channel_map[j];
                    self.inputs[index][..channel.len()].copy_from_slice(channel);
                }
            } else if let Some(channels) = port.channels()?.into_f64() {
                for (j, channel) in channels.iter().enumerate() {
                    let index = P::Plugin::INPUT_LAYOUT[i].channel_map[j];
                    for i in 0..channel.len() {
                        self.inputs[index][i] = channel[i] as f32;
                    }
                }
            } else {
                return Err(PluginError::Message("Unsupported input channel type"));
            }
        }
        Ok(())
    }

    fn copy_outputs(&mut self, audio: &mut Audio) -> Result<(), PluginError> {
        for (i, mut port) in audio.output_ports().enumerate() {
            if let Some(mut channels) = port.channels()?.into_f32() {
                for (j, channel) in channels.iter_mut().enumerate() {
                    let index = P::Plugin::OUTPUT_LAYOUT[i].channel_map[j];
                    channel.copy_from_slice(&self.outputs[index][..channel.len()]);
                }
            } else if let Some(mut channels) = port.channels()?.into_f64() {
                for (j, channel) in channels.iter_mut().enumerate() {
                    let index = P::Plugin::OUTPUT_LAYOUT[i].channel_map[j];
                    for i in 0..channel.len() {
                        channel[i] = self.outputs[index][i] as f64;
                    }
                }
            }
        }
        Ok(())
    }

    fn process_events(&mut self, events: InputEventsIter) {
        for event in events {
            if let Some(ev) = event.as_event::<ParamValueEvent>() {
                let Some(index) = ev.param_id().and_then(|id| {
                    let index = id.get() as usize;
                    if index < count::<P::Params>() {
                        Some(P::Params::from_usize(index))
                    } else {
                        None
                    }
                }) else {
                    continue;
                };
                self.dsp.set_param(index, ev.value() as _);
            }
        }
    }

    fn process_audio(&mut self, index: ops::Range<usize>) -> Result<ProcessStatus, PluginError> {
        let inputs = EnumMapArray::new(|i| &self.inputs[i][index.clone()]);
        // Safety: self.outputs is not aliased in this scope (self is already exclusively borrowed, and self.outputs
        // is not accessed anywhere else)
        let mut outputs = EnumMapArray::new(|i| unsafe { Indexed::new(&self.outputs[i], index.clone()) });
        self.dsp.process(index.len(), inputs.to_ref(), outputs.to_mut())
    }
}

impl<P: PluginDsp> PluginAudioProcessorParams for Processor<P> {
    fn flush(&mut self, input_parameter_changes: &InputEvents, _: &mut OutputEvents) {
        self.process_events(input_parameter_changes.iter());
    }
}

struct Indexed<T, I> {
    inner: *mut T,
    inner_len: usize,
    index: I,
}

impl<T, I> Indexed<T, I> {
    // Safety: This hides a mutable borrow as a shared one, it is manually needed to check that the entire passed
    // slice is not borrowed elsewhere.
    unsafe fn new(slice: &[T], index: I) -> Self {
        Self {
            inner: slice.as_ptr().cast_mut(),
            inner_len: slice.len(),
            index,
        }
    }
}

impl<'a, T, I: Clone> ops::Deref for Indexed<T, I>
where
    [T]: ops::Index<I>,
{
    type Target = <[T] as ops::Index<I>>::Output;

    fn deref(&self) -> &Self::Target {
        let slice = unsafe { std::slice::from_raw_parts(self.inner.cast(), self.inner_len) };
        &slice[self.index.clone()]
    }
}

impl<'a, T, I: Clone> ops::DerefMut for Indexed<T, I>
where
    [T]: ops::IndexMut<I>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        let slice = unsafe { std::slice::from_raw_parts_mut(self.inner.cast(), self.inner_len) };
        &mut slice[self.index.clone()]
    }
}
