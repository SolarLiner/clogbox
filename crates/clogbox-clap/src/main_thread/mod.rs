use crate::notifier::Notifier;
use crate::params::{ParamChangeEvent, ParamIdExt};
use crate::params::{ParamChangeKind, ParamId, ParamStorage};
#[cfg(feature = "gui")]
use crate::params::{ParamListener, ParamNotifier};
use crate::processor::PluginDsp;
use crate::shared::Shared;
use bincode::de::Decoder;
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use clack_extensions::audio_ports::{
    AudioPortFlags, AudioPortInfo, AudioPortInfoWriter, AudioPortType, PluginAudioPortsImpl,
};
use clack_extensions::params::{ParamDisplayWriter, ParamInfo, ParamInfoWriter, PluginMainThreadParams};
use clack_extensions::state::PluginStateImpl;
use clack_plugin::events::event_types::ParamValueEvent;
use clack_plugin::prelude::*;
use clack_plugin::stream::{InputStream, OutputStream};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{count, Enum, Mono, Stereo};
use clogbox_module::Module;
use std::ffi::CStr;
use std::fmt::Write;

#[cfg(not(feature = "gui"))]
type GuiHandle<E> = std::marker::PhantomData<E>;

#[cfg(feature = "gui")]
use super::gui::GuiHandle;

mod log;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PortLayout<E: 'static> {
    pub name: &'static str,
    pub main: bool,
    pub channel_map: &'static [E],
}

impl<E: 'static> PortLayout<E> {
    pub const fn main(self) -> Self {
        Self { main: true, ..self }
    }

    pub const fn named(self, name: &'static str) -> Self {
        Self { name, ..self }
    }
}

impl PortLayout<Mono> {
    pub const MONO: Self = Self {
        name: "Mono",
        main: false,
        channel_map: &[Mono],
    };
}

impl PortLayout<Stereo> {
    pub const STEREO: Self = Self {
        name: "Stereo",
        main: false,
        channel_map: &[Stereo::Left, Stereo::Right],
    };
}

pub trait Plugin: 'static + Sized {
    type Dsp: PluginDsp<Plugin = Self, ParamsIn = Self::Params>;
    type Params: ParamId;
    type SharedData: 'static + Clone + Send + Sync;

    const INPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioIn>];
    const OUTPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as Module>::AudioOut>];

    fn create(host: HostSharedHandle) -> Result<Self, PluginError>;

    fn shared_data(host: HostSharedHandle) -> Result<Self::SharedData, PluginError>;

    #[cfg(feature = "gui")]
    fn view(
        &mut self,
    ) -> Result<Box<dyn crate::gui::PluginView<Params = Self::Params, SharedData = Self::SharedData>>, PluginError>;
}

pub struct MainThread<'host, P: Plugin> {
    pub(crate) host: HostMainThreadHandle<'host>,
    pub(crate) shared: Shared<P>,
    pub(crate) gui: GuiHandle<P>,
    pub(crate) plugin: P,
    log_extension: Option<log::LogExtension>,
}

impl<'host, P: Plugin> MainThread<'host, P> {
    pub(crate) fn new(mut host: HostMainThreadHandle<'host>, shared: &Shared<P>) -> Result<Self, PluginError> {
        let plugin = P::create(host.shared())?;
        #[cfg(feature = "log")]
        let log_extension = log::init(&mut host);
        Ok(Self {
            host,
            shared: shared.clone(),
            gui: GuiHandle::default(),
            plugin,
            #[cfg(feature = "log")]
            log_extension,
        })
    }
}

impl<P: Plugin> PluginMainThreadParams for MainThread<'_, P> {
    fn count(&mut self) -> u32 {
        count::<P::Params>() as _
    }

    fn get_info(&mut self, param_index: u32, info: &mut ParamInfoWriter) {
        let index = param_index as usize;
        if index < count::<P::Params>() {
            let p = P::Params::from_usize(index);
            let range = if let Some(num_values) = p.discrete() {
                0.0..(num_values - 1) as _
            } else {
                0.0..1.0
            };
            let name = p.name();
            info.set(&ParamInfo {
                id: ClapId::new(param_index),
                flags: p.flags(),
                cookie: Default::default(),
                name: name.as_bytes(),
                module: b"",
                min_value: range.start as _,
                max_value: range.end as _,
                default_value: p.denormalized_to_clap_value(p.default_value()),
            });
        }
    }

    fn get_value(&mut self, param_id: ClapId) -> Option<f64> {
        let index = param_id.get() as usize;
        if index < count::<P::Params>() {
            Some(self.shared.params.get_clap_value(P::Params::from_usize(index)))
        } else {
            None
        }
    }

    fn value_to_text(&mut self, param_id: ClapId, value: f64, writer: &mut ParamDisplayWriter) -> std::fmt::Result {
        let index = param_id.get() as usize;
        if index < count::<P::Params>() {
            let param = P::Params::from_usize(index);
            param.value_to_text(writer, param.clap_value_to_denormalized(value))
        } else {
            writer.write_str("ERROR: Invalid parameter index")
        }
    }

    fn text_to_value(&mut self, param_id: ClapId, text: &CStr) -> Option<f64> {
        let index = param_id.get() as usize;
        if index < count::<P::Params>() {
            let text = text.to_string_lossy();
            let param = P::Params::from_usize(index);
            let mapping = param.mapping();
            param.text_to_value(&text).map(|v| mapping.normalize(v) as _)
        } else {
            None
        }
    }

    fn flush(&mut self, events: &InputEvents, _: &mut OutputEvents) {
        ::log::debug!("MainThread::flush");
        for event in events {
            let Some(event) = event.as_event::<ParamValueEvent>() else {
                continue;
            };
            let Some(id) = event.param_id().and_then(|id| {
                let index = id.get() as usize;
                (index < count::<P::Params>()).then(|| P::Params::from_usize(index))
            }) else {
                continue;
            };
            self.shared.params.set_clap_value(id, event.value());
            self.shared.notifier.notify(ParamChangeEvent {
                id,
                kind: ParamChangeKind::ValueChange(id.clap_value_to_denormalized(event.value())),
            });
        }
    }
}

impl<'a, P: Plugin + 'a> PluginMainThread<'a, Shared<P>> for MainThread<'a, P> {}

impl<P: Plugin> PluginAudioPortsImpl for MainThread<'_, P> {
    fn count(&mut self, is_input: bool) -> u32 {
        if is_input {
            P::INPUT_LAYOUT.len() as _
        } else {
            P::OUTPUT_LAYOUT.len() as _
        }
    }

    fn get(&mut self, index: u32, is_input: bool, writer: &mut AudioPortInfoWriter) {
        fn write_port_info<E>(writer: &mut AudioPortInfoWriter, index: u32, layout: PortLayout<E>) {
            let is_main = if layout.main {
                AudioPortFlags::IS_MAIN
            } else {
                AudioPortFlags::empty()
            };
            writer.set(&AudioPortInfo {
                id: ClapId::new(index),
                name: layout.name.as_bytes(),
                channel_count: layout.channel_map.len() as _,
                flags: AudioPortFlags::SUPPORTS_64BITS | is_main,
                port_type: match layout.channel_map.len() {
                    1 => Some(AudioPortType::MONO),
                    2 => Some(AudioPortType::STEREO),
                    _ => None,
                },
                in_place_pair: None,
            });
        }

        if is_input {
            write_port_info(writer, index, P::INPUT_LAYOUT[index as usize]);
        } else {
            let layout = P::OUTPUT_LAYOUT[index as usize];
            write_port_info(writer, index, layout);
        }
    }
}

struct Encode<'a, E: Enum> {
    params: &'a ParamStorage<E>,
    #[cfg(feature = "gui")]
    gui: Option<serde_json::Value>,
}

impl<E: Enum> bincode::Encode for Encode<'_, E> {
    fn encode<Enc: Encoder>(&self, encoder: &mut Enc) -> Result<(), EncodeError> {
        for (e, v) in self.params.read_all_values() {
            bincode::Encode::encode(&e.to_usize(), encoder)?;
            bincode::Encode::encode(&v, encoder)?;
        }
        #[cfg(feature = "gui")]
        {
            bincode::serde::Compat(&self.gui).encode(encoder)?;
        }
        Ok(())
    }
}

#[cfg(feature = "log")]
impl<P: Plugin> clack_extensions::timer::PluginTimerImpl for MainThread<'_, P> {
    fn on_timer(&mut self, timer_id: clack_extensions::timer::TimerId) {
        if let Some(ext) = &mut self.log_extension {
            ext.on_timer(self.host.shared(), timer_id);
        }
    }
}

struct Decode<E: Enum> {
    params: EnumMapArray<E, f32>,
    #[cfg(feature = "gui")]
    gui: Option<serde_json::Value>,
}

impl<E: Enum, Context> bincode::Decode<Context> for Decode<E> {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let mut params = EnumMapArray::new(|_| 0.0);
        for _ in 0..count::<E>() {
            let e = E::from_usize(usize::decode(decoder)?);
            params[e] = f32::decode(decoder)?;
        }
        #[cfg(feature = "gui")]
        {
            let gui = bincode::serde::Compat::<Option<serde_json::Value>>::decode(decoder)?.0;
            Ok(Self { params, gui })
        }
        #[cfg(not(feature = "gui"))]
        {
            Ok(Self { params })
        }
    }
}

impl<P: Plugin> PluginStateImpl for MainThread<'_, P> {
    fn save(&mut self, output: &mut OutputStream) -> Result<(), PluginError> {
        bincode::encode_into_std_write(
            Encode {
                params: &self.shared.params,
                #[cfg(feature = "gui")]
                gui: self.gui.save()?,
            },
            output,
            bincode::config::standard(),
        )?;
        Ok(())
    }

    fn load(&mut self, input: &mut InputStream) -> Result<(), PluginError> {
        let Decode {
            params,
            #[cfg(feature = "gui")]
            gui,
        } = bincode::decode_from_std_read(input, bincode::config::standard())?;
        self.shared.params.store_all_values(params);
        #[cfg(feature = "gui")]
        if let Some(gui) = gui {
            self.gui.load(gui)?;
        }
        Ok(())
    }
}
