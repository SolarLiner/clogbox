use clack_extensions::audio_ports::{AudioPortFlags, AudioPortInfo, AudioPortType};
use clack_extensions::params::{ParamDisplayWriter, ParamInfo, ParamInfoFlags, ParamInfoWriter};
use clack_extensions::{audio_ports, params};
use clack_plugin::events::event_types::ParamValueEvent;
use clack_plugin::events::spaces::CoreEventSpace;
use clack_plugin::prelude::*;
use clogbox_core::module::{
    BufferStorage, Module, ModuleConstructor, ModuleContext, RawModule, StreamData,
};
use clogbox_core::param::events::{ParamEvents, ParamEventsMut, ParamSlice};
use clogbox_core::param::{Normalized, ParamFlags, Params};
use clogbox_core::r#enum::{count, Enum};
use std::ffi::CStr;
use std::fmt::Write;
use std::marker::PhantomData;

pub use clack_plugin::host::HostSharedHandle;
use clack_plugin::plugin;
pub use clack_plugin::plugin::PluginError;
pub use clack_plugin::{clack_export_entry, plugin::features, plugin::PluginDescriptor};
use clogbox_core::r#enum::az::CastFrom;

#[macro_export]
macro_rules! clap_module {
    ($ctor:ty) => {
        const fn __ensure_is_clap_module<Ctor: $crate::ClapModule>() -> bool {
            true
        }

        const _: () = {
            // This is a hack to ensure that the type is a ClapModule, checked at compile-time.
            let _ = __ensure_is_clap_module::<$ctor>();
        };

        pub struct __ClapModule {
            constructor: $ctor,
        }

        impl Plugin for __ClapModule {
            type AudioProcessor<'a> = WrapperProcessor<$ctor::Module>;
            type Shared<'a> = WrapperShared<$ctor>;
            type MainThread<'a> = $crate::ClapWrapperMainThread<
                <$ctor as clogbox_core::module::ModuleConstructor>::Module,
            >;

            fn declare_extensions(
                builder: &mut PluginExtensions<Self>,
                shared: Option<&Self::Shared<'_>>,
            ) {
                $crate::declare_extensions(builder)
            }
        }

        impl DefaultPluginFactory for __ClapModule {
            fn get_descriptor() -> PluginDescriptor {
                Ctor::descriptor()
            }

            fn new_shared(host: HostSharedHandle) -> Result<Self::Shared<'_>, PluginError> {
                let constructor = $ctor::create(host)?;
                Ok(WrapperShared { constructor })
            }

            fn new_main_thread<'a>(
                host: HostMainThreadHandle<'a>,
                shared: &'a Self::Shared<'a>,
            ) -> Result<Self::MainThread<'a>, PluginError> {
                Ok(())
            }
        }

        $crate::clack_export_entry!(__ClapModule);
    };
}
type CtorParams<Ctor> = <<Ctor as ModuleConstructor>::Module as Module>::Params;

pub fn declare_extensions<This: Plugin>(builder: &mut PluginExtensions<This>)
where
    for<'a> <This as Plugin>::MainThread<'a>: audio_ports::PluginAudioPortsImpl
        + clack_extensions::params::PluginMainThreadParams
        + clack_extensions::state::PluginStateImpl,
    for<'a> <This as Plugin>::AudioProcessor<'a>:
        clack_extensions::params::PluginAudioProcessorParams,
{
    use clack_extensions::{audio_ports::*, params::*, state::*};
    builder
        .register::<PluginAudioPorts>()
        .register::<PluginParams>()
        .register::<PluginState>();
}

pub trait ClapModule:
    'static + Sized + Send + Sync + ModuleConstructor<Module: Module<Sample = f32>>
{
    fn descriptor() -> PluginDescriptor;
    fn create(host: HostSharedHandle) -> Result<Self, PluginError>;
}

pub struct WrapperProcessor<M> {
    stream_data: StreamData,
    module: M,
    params: Box<[Box<ParamSlice>]>,
}

impl<
        'a,
        M: 'static + Module<Sample = f32>,
        Ctor: 'static + Send + Sync + ModuleConstructor<Module = M>,
    > PluginAudioProcessor<'a, WrapperShared<Ctor>, ClapWrapperMainThread<Ctor>>
    for WrapperProcessor<M>
{
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main_thread: &mut ClapWrapperMainThread<Ctor>,
        shared: &'a WrapperShared<Ctor>,
        audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        let stream_data = StreamData {
            sample_rate: audio_config.sample_rate,
            block_size: audio_config.max_frames_count as usize,
            bpm: f64::NAN,
        };
        let module = shared.constructor.allocate(stream_data);
        let params = std::iter::repeat_with(|| {
            std::iter::repeat_with(|| (0, 0.0))
                .take(stream_data.block_size)
                .collect()
        })
        .take(module.params())
        .collect();
        Ok(Self {
            stream_data,
            module,
            params,
        })
    }

    fn process(
        &mut self,
        _process: Process,
        audio: Audio,
        events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        for event in events.input {
            if let Some(CoreEventSpace::Transport(transport)) = event.as_core_event() {
                self.stream_data.bpm = transport.tempo;
            }
        }
        let mut storage = ClapBufferStorage { audio };

        for event in events.input {
            let Some(value_event) = event.as_event::<ParamValueEvent>() else {
                continue;
            };
            let Some(id) = value_event.param_id().map(|id| id.get() as usize) else {
                continue;
            };
            self.params.set_event(id, value_event.value());
        }

        <M as RawModule>::process(
            &mut self.module,
            &mut ModuleContext {
                buffers: &mut storage,
                stream_data: self.stream_data,
            },
            &self.params,
        );

        Ok(ProcessStatus::Continue)
    }
}

pub struct WrapperShared<Ctor> {
    constructor: Ctor,
}

impl<Ctor: 'static + Send + Sync> PluginShared<'_> for WrapperShared<Ctor> {}

pub struct ClapWrapperMainThread<Ctor> {
    __params: PhantomData<Ctor>,
}

impl<Ctor: ModuleConstructor + std::marker::Send + std::marker::Sync + 'static>
    plugin::PluginMainThread<'_, WrapperShared<Ctor>> for ClapWrapperMainThread<Ctor>
{
}

impl<Ctor: ModuleConstructor> params::PluginMainThreadParams for ClapWrapperMainThread<Ctor> {
    fn count(&mut self) -> u32 {
        count::<<Ctor::Module as Module>::Params>() as _
    }

    fn get_info(&mut self, param_index: u32, info: &mut ParamInfoWriter) {
        if param_index < count::<CtorParams<Ctor>>() as u32 {
            let param = CtorParams::<Ctor>::cast_from(param_index as usize);
            let name = param.name();
            let metadata = param.metadata();
            info.set(&ParamInfo {
                id: ClapId::new(param_index),
                name: name.as_bytes(),
                module: &[],
                min_value: *metadata.range.start() as _,
                max_value: *metadata.range.end() as _,
                flags: clogbox_flag_to_clap_flag(metadata.flags),
                cookie: Default::default(),
                default_value: 0.0,
            });
        }
    }

    fn get_value(&mut self, param_id: ClapId) -> Option<f64> {
        todo!()
    }

    fn value_to_text(
        &mut self,
        param_id: ClapId,
        value: f64,
        writer: &mut ParamDisplayWriter,
    ) -> std::fmt::Result {
        let s = CtorParams::<Ctor>::cast_from(param_id.get() as _)
            .value_to_string(Normalized::new(value as _).unwrap());
        writer.write_str(&s)
    }

    fn text_to_value(&mut self, param_id: ClapId, text: &CStr) -> Option<f64> {
        let s = text.to_string_lossy();
        CtorParams::<Ctor>::cast_from(param_id.get() as _)
            .string_to_value(&s)
            .ok()
            .map(|n| n.into_inner() as _)
    }

    fn flush(
        &mut self,
        input_parameter_changes: &InputEvents,
        output_parameter_changes: &mut OutputEvents,
    ) {
    }
}

fn clogbox_flag_to_clap_flag(flag: ParamFlags) -> ParamInfoFlags {
    let mut out = ParamInfoFlags::empty();
    if flag.contains(ParamFlags::MODULABLE) {
        out |= ParamInfoFlags::IS_MODULATABLE;
    }
    if flag.contains(ParamFlags::AUTOMATABLE) {
        out |= ParamInfoFlags::IS_AUTOMATABLE;
    }
    out
}

impl<Ctor: ModuleConstructor> audio_ports::PluginAudioPortsImpl for ClapWrapperMainThread<Ctor> {
    fn count(&mut self, is_input: bool) -> u32 {
        if is_input {
            count::<<Ctor::Module as Module>::Inputs>() as _
        } else {
            count::<<Ctor::Module as Module>::Outputs>() as _
        }
    }

    fn get(&mut self, index: u32, is_input: bool, writer: &mut audio_ports::AudioPortInfoWriter) {
        if is_input {
            let inputs = count::<<Ctor::Module as Module>::Inputs>() as u32;
            if inputs > 0 {
                writer.set(&AudioPortInfo {
                    id: ClapId::new(0),
                    name: b"Input",
                    channel_count: inputs,
                    flags: AudioPortFlags::IS_MAIN | AudioPortFlags::REQUIRES_COMMON_SAMPLE_SIZE,
                    in_place_pair: Some(ClapId::new(0)),
                    port_type: AudioPortType::from_channel_count(inputs),
                });
            }
        } else {
            let outputs = count::<<Ctor::Module as Module>::Outputs>() as u32;
            if outputs > 0 {
                writer.set(&AudioPortInfo {
                    id: ClapId::new(0),
                    name: b"Output",
                    channel_count: outputs,
                    flags: AudioPortFlags::IS_MAIN | AudioPortFlags::REQUIRES_COMMON_SAMPLE_SIZE,
                    in_place_pair: Some(ClapId::new(0)),
                    port_type: AudioPortType::from_channel_count(outputs),
                });
            }
        }
    }
}

struct ClapBufferStorage<'a> {
    audio: Audio<'a>,
}

impl BufferStorage for ClapBufferStorage<'_> {
    type Sample = f32;
    type Input = usize;
    type Output = usize;

    fn get_input_buffer(&self, input: Self::Input) -> &[Self::Sample] {
        self.audio
            .input_port(0)
            .expect("Input port 0 not found")
            .channels()
            .expect("Input port 0 has no channels")
            .as_f32()
            .expect("Input port 0 is not f32")
            .channel(input as _)
            .expect("Input channel not found")
    }

    fn get_output_buffer(&mut self, output: Self::Output) -> &mut [Self::Sample] {
        let mut channels = self
            .audio
            .output_port(0)
            .expect("Output port 0 not found")
            .channels()
            .unwrap()
            .into_f32()
            .unwrap();
        let slice = channels.channel_mut(output as u32).unwrap();
        let ptr = slice.as_mut_ptr();
        // Safety: we need to detach the slice lifetime from the port lifetime, as Rust doesn't see
        // through the many temporary objects we've created.
        // This is safe because Rust will tie the lifetime of this slice to the lifetime of Self.
        // This is sad and should be fixed in some fashion, but I have not had the correct stroke of genius to do so.
        unsafe { std::slice::from_raw_parts_mut(ptr, slice.len()) }
    }

    fn get_inout_pair(
        &mut self,
        input: Self::Input,
        output: Self::Output,
    ) -> (&[Self::Sample], &mut [Self::Sample]) {
        let output = {
            let mut channels = self
                .audio
                .output_port(0)
                .expect("Output port 0 not found")
                .channels()
                .unwrap()
                .into_f32()
                .unwrap();
            let channel = channels.channel_mut(output as u32).unwrap();
            let ptr = channel.as_mut_ptr();
            // Safety: we need to detach the slice lifetime from the port lifetime, as Rust doesn't see
            // through the many temporary objects we've created.
            // This is safe because Rust will tie the lifetime of this slice to the lifetime of Self.
            // This is sad and should be fixed in some fashion, but I have not had the correct stroke of genius to do so.
            unsafe { std::slice::from_raw_parts_mut(ptr, channel.len()) }
        };
        let input = self
            .audio
            .input_port(0)
            .expect("Input port 0 not found")
            .channels()
            .expect("Input port 0 has no channels")
            .as_f32()
            .expect("Input port 0 is not f32")
            .channel(input as _)
            .expect("Input channel not found");
        (input, output)
    }

    fn reset(&mut self) {}

    fn clear_input(&mut self, _input: Self::Input) {}

    fn clear_output(&mut self, _output: Self::Output) {}
}
