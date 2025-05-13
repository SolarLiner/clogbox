use crate::GetContextExtra;
use clogbox_clap::params::ParamId;
use clogbox_utils::AtomicF32;
use egui::InnerResponse;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Component helper refreshing drawing periodically to update an ui based on the provided value
pub struct Driven {
    /// Value to pass through into [`Self::show`]
    pub by: f32,
    /// Duration to wait before the next refresh
    pub refresh_after: Duration,
}

impl Driven {
    /// Create a [`Driven`] from this float value
    pub fn by_float(by: f32) -> Self {
        Self {
            by,
            refresh_after: Duration::from_nanos(16_666_667),
        }
    }

    /// Create a [`Driven`] from loading an atomic value
    pub fn by_atomic(atomic: &AtomicF32) -> Self {
        Self::by_float(atomic.load(Ordering::Relaxed))
    }

    /// Create a [`Driven`] from loading a parameter value
    pub fn by_param<E: ParamId>(ctx: &impl GetContextExtra, param: E) -> Self {
        Self::by_float(ctx.plugin_gui_context::<E>().params[param].get())
    }

    /// Change the duration before the next refresh. Note that a refresh may happen before this duration if another
    /// refresh is requested with an earlier duration.
    pub fn refresh_after(mut self, refresh_after: Duration) -> Self {
        self.refresh_after = refresh_after;
        self
    }
}

impl Driven {
    /// Drive the inner ui by passing it the value and requesting a repaint to provide continuous updates.
    pub fn show<R, F: FnMut(&mut egui::Ui, f32) -> R>(&mut self, ui: &mut egui::Ui, mut func: F) -> InnerResponse<R> {
        ui.allocate_new_ui(egui::UiBuilder::new(), move |ui| {
            let ret = func(ui, self.by);
            ui.ctx().request_repaint_after(Duration::from_nanos(16_666_667));
            ret
        })
    }
}
