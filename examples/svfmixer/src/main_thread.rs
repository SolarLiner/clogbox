use crate::params::ParamId;
use crate::shared;
use clack_extensions::audio_ports::{
    AudioPortFlags, AudioPortInfo, AudioPortInfoWriter, PluginAudioPortsImpl,
};
use clack_extensions::params::{ParamDisplayWriter, ParamInfoWriter, PluginMainThreadParams};
use clack_extensions::state::PluginStateImpl;
use clack_plugin::prelude::*;
use clack_plugin::stream::{InputStream, OutputStream};
use clogbox_enum::{count, enum_iter, Enum};
use std::ffi::CStr;
use std::io::{Read, Write};

pub struct SvfMixerMainThread {
    pub(crate) shared: shared::SvfMixerShared,
}

impl PluginMainThreadParams for SvfMixerMainThread {
    fn count(&mut self) -> u32 {
        count::<ParamId>() as _
    }

    fn get_info(&mut self, param_index: u32, info: &mut ParamInfoWriter) {
        let index = param_index as usize;
        if index >= count::<ParamId>() {
            return;
        }
        let id = ParamId::from_usize(index);
        id.write_param_info(info);
    }

    fn get_value(&mut self, param_id: ClapId) -> Option<f64> {
        let index = param_id.get() as usize;
        if index >= count::<ParamId>() {
            return None;
        }

        let id = ParamId::from_usize(index);
        Some(self.shared.params.get_param_normalized(id))
    }

    fn value_to_text(
        &mut self,
        param_id: ClapId,
        value: f64,
        writer: &mut ParamDisplayWriter,
    ) -> std::fmt::Result {
        let index = param_id.get() as usize;
        if index >= count::<ParamId>() {
            return Ok(());
        }
        let id = ParamId::from_usize(index);
        id.display_value(writer, value)
    }

    fn text_to_value(&mut self, param_id: ClapId, text: &CStr) -> Option<f64> {
        let index = param_id.get() as usize;
        if index >= count::<ParamId>() {
            return None;
        }
        let id = ParamId::from_usize(index);
        let value = text.to_string_lossy().parse().ok()?;
        Some(id.clamp_value(value))
    }

    fn flush(
        &mut self,
        input_parameter_changes: &InputEvents,
        output_parameter_changes: &mut OutputEvents,
    ) {
    }
}

impl PluginMainThread<'_, shared::SvfMixerShared> for SvfMixerMainThread {}

impl PluginAudioPortsImpl for SvfMixerMainThread {
    fn count(&mut self, is_input: bool) -> u32 {
        1
    }

    fn get(&mut self, index: u32, is_input: bool, writer: &mut AudioPortInfoWriter) {
        if index >= 1 {
            return;
        }

        let name = if is_input {
            b"Input" as &[u8]
        } else {
            b"Output"
        };
        let flags = AudioPortFlags::IS_MAIN;
        writer.set(&AudioPortInfo {
            id: ClapId::new(index as _),
            name,
            channel_count: 2,
            flags,
            port_type: None,
            in_place_pair: None,
        });
    }
}

impl PluginStateImpl for SvfMixerMainThread {
    fn save(&mut self, output: &mut OutputStream) -> Result<(), PluginError> {
        for id in enum_iter::<ParamId>() {
            let value = self.shared.params.get_param(id);
            output.write_all(&value.to_le_bytes())?;
        }
        Ok(())
    }

    fn load(&mut self, input: &mut InputStream) -> Result<(), PluginError> {
        for id in enum_iter::<ParamId>() {
            let mut data = f32::to_le_bytes(0.0);
            input.read_exact(&mut data)?;
            let value = f32::from_le_bytes(data);
            self.shared.params.set_param(id, value);
        }
        Ok(())
    }
}
