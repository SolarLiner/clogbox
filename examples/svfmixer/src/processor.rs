use crate::{main_thread, params, shared};
use clack_extensions::params::PluginAudioProcessorParams;
use clack_plugin::events::event_types::ParamValueEvent;
use clack_plugin::host::HostAudioProcessorHandle;
use clack_plugin::plugin::PluginError;
use clack_plugin::prelude::*;
use clogbox_core::math::interpolation;
use clogbox_core::smoothers::{LinearSmoother, Smoother};
use clogbox_enum::enum_map::EnumMapArray;
use clogbox_enum::{count, Enum};
use clogbox_filters::svf::Svf;
use clogbox_filters::{sinh, SimpleSaturator};
use std::array;

pub struct SvfMixerProcessor {
    params: params::Storage,
    smoothers: EnumMapArray<params::ParamId, LinearSmoother<f32>>,
    dsp: [Svf<SimpleSaturator<f32>>; 2],
}

impl PluginAudioProcessor<'_, shared::SvfMixerShared, main_thread::SvfMixerMainThread>
    for SvfMixerProcessor
{
    fn activate(
        host: HostAudioProcessorHandle<'_>,
        main_thread: &mut main_thread::SvfMixerMainThread,
        shared: &'_ shared::SvfMixerShared,
        audio_config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        use params::ParamId::*;
        let params = shared.params.clone();
        let smoothers = EnumMapArray::new(|p| {
            LinearSmoother::new(
                interpolation::Linear,
                audio_config.sample_rate as _,
                10e-3,
                params.get_param(p),
                params.get_param(p),
            )
        });
        let dsp = array::from_fn(|_| {
            Svf::new(
                audio_config.sample_rate as _,
                params.get_param(Cutoff),
                params.get_param(Resonance),
            )
            .with_saturator(sinh())
        });
        Ok(Self {
            params,
            smoothers,
            dsp,
        })
    }

    fn process(
        &mut self,
        process: Process,
        mut audio: Audio,
        events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        use params::ParamId::*;
        let buffer_size = audio.frames_count() as usize;
        let mut port_pair = audio
            .port_pair(0)
            .ok_or(PluginError::Message("Audio port configuration incorrect"))?;
        let mut channels = port_pair.channels()?;
        let channels = channels.as_f32_mut().ok_or(PluginError::Message(
            "Cannot process: float data unavailable",
        ))?;

        for batch in events.input.batch() {
            for event in batch.events() {
                self.update_from_event(event);
            }
            let range =
                batch.first_sample()..batch.next_batch_first_sample().unwrap_or(buffer_size);
            for i in range {
                for (mut channel_pair, svf) in channels.iter_mut().zip(&mut self.dsp) {
                    svf.set_resonance_no_update(self.smoothers[Resonance].next_value());
                    svf.set_cutoff(self.smoothers[Cutoff].next_value());
                    let (lp, bp, hp) = svf.next_sample(channel_pair.input().unwrap()[i]);
                    let y = lp * self.smoothers[Lowpass].next_value()
                        + bp * self.smoothers[Bandpass].next_value()
                        + hp * self.smoothers[Highpass].next_value();
                    channel_pair.output_mut().unwrap()[i] = y;
                }
            }
        }
        Ok(ProcessStatus::ContinueIfNotQuiet)
    }
}

impl PluginAudioProcessorParams for SvfMixerProcessor {
    fn flush(
        &mut self,
        input_parameter_changes: &InputEvents,
        output_parameter_changes: &mut OutputEvents,
    ) {
    }
}

impl SvfMixerProcessor {
    fn update_from_event(&mut self, event: &UnknownEvent) {
        if let Some(event) = event.as_event::<ParamValueEvent>() {
            if let Some(param_id) = event
                .param_id()
                .into_iter()
                .find_map(|id| {
                    let index = id.get() as usize;
                    (index < count::<params::ParamId>())
                        .then(|| params::ParamId::from_usize(index))
                })
            {
                let value = event.value() as _;
                self.params.set_param(param_id, value);
                self.smoothers[param_id].set_target(value);
                println!("{:?} = {}", param_id, value);
            }
        }
    }
}
