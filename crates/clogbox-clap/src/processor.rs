use crate::main_thread::{MainThread, Plugin};
use crate::params::ParamId;
use crate::shared::Shared;
use clack_extensions::params::PluginAudioProcessorParams;
use clack_plugin::events::event_types::ParamValueEvent;
use clack_plugin::host::HostAudioProcessorHandle;
pub use clack_plugin::host::HostSharedHandle;
pub use clack_plugin::plugin::PluginError;
use clack_plugin::prelude::*;
pub use clack_plugin::process::{PluginAudioConfiguration, ProcessStatus};
use clogbox_enum::enum_map::{EnumMapArray, EnumMapRef};
use clogbox_enum::{count, enum_iter, Empty, Enum};
use clogbox_module::eventbuffer::{EventBuffer, EventSlice, Timestamped};
use clogbox_module::{Module, ProcessContext, Samplerate, StreamContext};
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::ops;

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

    fn create(context: PluginCreateContext<Self>) -> Self;
}

#[derive(Debug, Clone)]
struct AudioStorage<E: Enum, T> {
    storage: EnumMapArray<E, Box<[T]>>,
}

impl<E: Enum, T> AudioStorage<E, T> {
    pub fn new(fill: impl Fn(E) -> Box<[T]>) -> Self {
        Self {
            storage: EnumMapArray::new(fill),
        }
    }

    pub fn default(capacity: usize) -> Self
    where
        T: Default,
    {
        Self::new(|_| Box::from_iter(std::iter::repeat_with(T::default).take(capacity)))
    }
}

impl<E: Enum, T> ops::Index<E> for AudioStorage<E, T> {
    type Output = [T];

    fn index(&self, index: E) -> &Self::Output {
        &self.storage[index][..]
    }
}

impl<E: Enum, T> ops::IndexMut<E> for AudioStorage<E, T> {
    fn index_mut(&mut self, index: E) -> &mut Self::Output {
        &mut self.storage[index][..]
    }
}

struct EventStorage<E: Enum, T> {
    storage: EnumMapArray<E, EventBuffer<T>>,
}

impl<E: Enum, T> ops::Index<E> for EventStorage<E, T> {
    type Output = EventSlice<T>;

    fn index(&self, index: E) -> &Self::Output {
        &self.storage[index].as_slice()
    }
}

impl<E: Enum, T> ops::IndexMut<E> for EventStorage<E, T> {
    fn index_mut(&mut self, index: E) -> &mut Self::Output {
        self.storage[index].as_mut_slice()
    }
}

impl<E: Enum, T> EventStorage<E, T> {
    pub fn new() -> Self {
        Self {
            storage: EnumMapArray::new(|_| EventBuffer::new()),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self
    where
        T: Default,
    {
        Self {
            storage: EnumMapArray::new(|_| EventBuffer::with_capacity(capacity)),
        }
    }
}

impl<T> EventStorage<Empty, T> {
    pub const fn empty() -> Self {
        Self {
            storage: EnumMapArray::CONST_DEFAULT,
        }
    }
}

pub struct Processor<'a, P: PluginDsp> {
    shared: &'a Shared<P::ParamsIn>,
    dsp: P,
    audio_in: AudioStorage<P::AudioIn, P::Sample>,
    audio_out: AudioStorage<P::AudioOut, P::Sample>,
    params: EventStorage<P::ParamsIn, f32>,
    sample_rate: Samplerate,
    tail: Option<NonZeroU32>,
}

impl<'a, P: 'a + PluginDsp> PluginAudioProcessor<'a, Shared<P::ParamsIn>, MainThread<P::Plugin>> for Processor<'a, P> {
    fn activate(
        host: HostAudioProcessorHandle<'a>,
        main_thread: &mut MainThread<P::Plugin>,
        shared: &'a Shared<P::ParamsIn>,
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
            processor_main_thread: &mut main_thread.processor_main_thread,
            audio_config,
        };
        let mut dsp = P::create(context);
        let audio_in = AudioStorage::default(audio_config.max_frames_count as usize);
        let audio_out = AudioStorage::default(audio_config.max_frames_count as usize);
        let params = EventStorage::with_capacity(512);
        dsp.prepare(Samplerate::new(audio_config.sample_rate), audio_config.max_frames_count as _);
        Ok(Self {
            shared,
            dsp,
            audio_in,
            audio_out,
            params,
            sample_rate: Samplerate::new(audio_config.sample_rate),
            tail: None,
        })
    }

    fn process(&mut self, _process: Process, mut audio: Audio, events: Events) -> Result<ProcessStatus, PluginError> {
        self.copy_inputs(&audio)?;
        self.copy_events(&events)?;
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

    fn copy_events(&mut self, events: &Events) -> Result<(), PluginError> {
        for buf in self.params.storage.values_mut() {
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
            let mapping = param.mapping();
            self.params.storage[param].push(ev.time() as _, mapping.denormalize(ev.value() as _));
        }
        // Send last param values to the shared state
        for param in enum_iter::<P::ParamsIn>() {
            let Some(&Timestamped { data: value, .. }) = self.params.storage[param].last() else { continue; };
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
