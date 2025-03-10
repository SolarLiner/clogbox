use crate::params::{ParamId, ParamStorage};
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
use clack_plugin::events::io::InputEventBuffer;
use clack_plugin::prelude::*;
use clack_plugin::stream::{InputStream, OutputStream};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{count, Enum, Mono, Stereo};
use std::ffi::CStr;
use std::fmt::Write;

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
    type Dsp: PluginDsp<Plugin = Self, Params = Self::Params>;
    type Params: ParamId;

    const INPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as PluginDsp>::Inputs>];
    const OUTPUT_LAYOUT: &'static [PortLayout<<Self::Dsp as PluginDsp>::Outputs>];

    fn create(host: HostSharedHandle) -> Result<Self, PluginError>;
}

pub struct MainThread<P: Plugin> {
    shared: Shared<P::Params>,
    pub(crate) processor_main_thread: P,
}

impl<P: Plugin> MainThread<P> {
    pub(crate) fn new(host: HostMainThreadHandle, shared: &Shared<P::Params>) -> Result<Self, PluginError> {
        let processor_main_thread = P::create(host.shared())?;
        Ok(Self {
            shared: shared.clone(),
            processor_main_thread,
        })
    }
}

impl<P: Plugin> PluginMainThreadParams for MainThread<P> {
    fn count(&mut self) -> u32 {
        count::<P::Params>() as _
    }

    fn get_info(&mut self, param_index: u32, info: &mut ParamInfoWriter) {
        let index = param_index as usize;
        if index < count::<P::Params>() {
            let p = P::Params::from_usize(index);
            let range = p.mapping().range();
            let name = p.name();
            info.set(&ParamInfo {
                id: ClapId::new(param_index),
                flags: p.flags(),
                cookie: Default::default(),
                name: name.as_bytes(),
                module: b"",
                min_value: range.start as _,
                max_value: range.end as _,
                default_value: p.default_value() as _,
            });
        }
    }

    fn get_value(&mut self, param_id: ClapId) -> Option<f64> {
        let index = param_id.get() as usize;
        if index < count::<P::Params>() {
            let id = P::Params::from_usize(index);
            Some(self.shared.params.get(id) as _)
        } else {
            None
        }
    }

    fn value_to_text(&mut self, param_id: ClapId, value: f64, writer: &mut ParamDisplayWriter) -> std::fmt::Result {
        let index = param_id.get() as usize;
        if index < count::<P::Params>() {
            P::Params::from_usize(index).value_to_text(writer, value as _)
        } else {
            writer.write_str("ERROR: Invalid parameter index")
        }
    }

    fn text_to_value(&mut self, param_id: ClapId, text: &CStr) -> Option<f64> {
        let index = param_id.get() as usize;
        if index < count::<P::Params>() {
            let text = text.to_string_lossy();
            P::Params::from_usize(index).text_to_value(&text).map(|v| v as _)
        } else {
            None
        }
    }

    fn flush(&mut self, events: &InputEvents, _: &mut OutputEvents) {
        for event in events {
            if let Some(event) = event.as_event::<ParamValueEvent>() {
                let id = event.param_id().into_iter().find_map(|id| {
                    let index = id.get() as usize;
                    (index < count::<P::Params>()).then(|| P::Params::from_usize(index))
                });
                if let Some(id) = id {
                    let value = event.value() as _;
                    self.shared.params.set(id, value);
                }
            }
        }
    }
}

impl<'a, P: Plugin + 'a> PluginMainThread<'a, Shared<P::Params>> for MainThread<P> {}

impl<P: Plugin> PluginAudioPortsImpl for MainThread<P> {
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

struct Encode<'a, E: Enum>(&'a ParamStorage<E>);

impl<E: Enum> bincode::Encode for Encode<'_, E> {
    fn encode<Enc: Encoder>(&self, encoder: &mut Enc) -> Result<(), EncodeError> {
        for (e, v) in self.0.read_all_values() {
            bincode::Encode::encode(&e.to_usize(), encoder)?;
            bincode::Encode::encode(&v, encoder)?;
        }
        Ok(())
    }
}

struct Decode<E: Enum>(EnumMapArray<E, f32>);

impl<E: Enum, Context> bincode::Decode<Context> for Decode<E> {
    fn decode<D: Decoder<Context = Context>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let mut values = EnumMapArray::new(|_| 0.0);
        for _ in 0..count::<E>() {
            let e = E::from_usize(usize::decode(decoder)?);
            values[e] = f32::decode(decoder)?;
        }
        Ok(Self(values))
    }
}

impl<P: Plugin> PluginStateImpl for MainThread<P> {
    fn save(&mut self, output: &mut OutputStream) -> Result<(), PluginError> {
        bincode::encode_into_std_write(Encode(&self.shared.params), output, bincode::config::standard())?;
        Ok(())
    }

    fn load(&mut self, input: &mut InputStream) -> Result<(), PluginError> {
        let Decode(values) = bincode::decode_from_std_read(input, bincode::config::standard())?;
        self.shared.params.store_all_values(values);
        Ok(())
    }
}
