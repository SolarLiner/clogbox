use crate::components::Knob;
use crate::{EguiPluginView, GetContextExtra};
use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::PluginView;
use clogbox_clap::params::{ParamChangeEvent, ParamChangeKind, ParamId};
use clogbox_clap::PluginError;
use clogbox_enum::enum_iter;
use egui::{emath, Align, ComboBox, Layout, Ui};
use std::marker::PhantomData;

/// Creates a generic UI for plugin parameters
///
/// Creates a default user interface that automatically displays all parameters
/// defined in the enum type `E`.
///
/// # Type Parameters
///
/// * `E` - Parameter ID enum that implements `ParamId`
/// * `SharedData` - Type for shared data between UI and audio processing
///
/// # Parameters
///
/// * `size` - Initial size of the UI window
///
/// # Returns
///
/// A boxed `PluginView` implementation or an error
pub fn generic_ui<E: ParamId, SharedData: 'static + Send + Sync + Clone>(
    size: GuiSize,
) -> Result<Box<dyn PluginView<Params = E, SharedData = SharedData>>, PluginError> {
    crate::view(size, GenericUi(PhantomData))
}

/// Internal implementation of generic UI
struct GenericUi<E: ParamId>(PhantomData<E>);

impl<E: ParamId> EguiPluginView for GenericUi<E> {
    type Params = E;

    fn update(&mut self, ctx: &egui::Context, _queue: &mut egui_baseview::Queue) {
        egui::CentralPanel::default().show(ctx, |ui| {
            display::<E>(ui);
        });
    }
}

/// Default size for knob UI elements
pub const KNOB_SIZE: f32 = 40.0;
/// Spacing between UI elements
const SPACING: f32 = 10.0;

/// Displays parameters with a custom display function
///
/// Creates a grid layout of parameters and calls the provided display function
/// for each parameter.
///
/// # Type Parameters
///
/// * `E` - Parameter ID enum that implements `ParamId`
///
/// # Parameters
///
/// * `ui` - The egui UI to render into
/// * `display_fn` - Function to display each parameter element
pub fn display_with<E: ParamId>(ui: &mut Ui, mut display_fn: impl FnMut(&mut Ui, emath::Rect, E)) {
    let element_width = 2.0 * KNOB_SIZE + SPACING;
    ui.style_mut().spacing.item_spacing = emath::vec2(SPACING, SPACING);
    let rect = ui.available_rect_before_wrap();
    let num_columns = (rect.width() / element_width).floor() as usize;
    egui::Grid::new("knobs").num_columns(num_columns).show(ui, |ui| {
        for (i, param) in enum_iter::<E>().enumerate() {
            let rect = {
                let mut r = ui.available_rect_before_wrap();
                r.set_width(element_width);
                r
            };
            display_fn(ui, rect, param);
            if i % num_columns == num_columns - 1 {
                ui.end_row();
            }
        }
    });
}

/// Displays all parameters using the default knob UI
///
/// Creates a grid of parameter knobs using the default visualization.
///
/// # Type Parameters
///
/// * `E` - Parameter ID enum that implements `ParamId`
///
/// # Parameters
///
/// * `ui` - The egui UI to render into
pub fn display<E: ParamId>(ui: &mut Ui) {
    display_with::<E>(ui, |ui, rect, param| show_knob(ui, rect.width(), param));
}

/// Displays a single parameter as a knob
///
/// Creates either a continuous or discrete knob depending on the parameter type.
///
/// # Type Parameters
///
/// * `E` - Parameter ID enum that implements `ParamId`
///
/// # Parameters
///
/// * `ui` - The egui UI to render into
/// * `element_width` - Width of the UI element
/// * `param` - The parameter to display
pub fn show_knob<E: ParamId>(ui: &mut Ui, element_width: f32, param: E) {
    let knob_width = element_width / 2.0;
    ui.allocate_ui_with_layout(
        emath::vec2(element_width, element_width),
        Layout::top_down(Align::Center).with_cross_align(Align::Center),
        |ui| {
            ui.set_min_width(element_width);
            if let Some(num_values) = param.discrete() {
                discrete_knob(ui, param, num_values);
            } else {
                continuous_knob(ui, param, knob_width);
            }
        },
    );
}

/// Creates a continuous parameter knob
///
/// Displays a rotary knob UI element for continuous parameters.
///
/// # Type Parameters
///
/// * `E` - Parameter ID enum that implements `ParamId`
///
/// # Parameters
///
/// * `ui` - The egui UI to render into
/// * `param` - The parameter to display
/// * `knob_width` - Width of the knob UI element
fn continuous_knob<E: ParamId>(ui: &mut Ui, param: E, knob_width: f32) {
    ui.add(Knob::new(ui.ctx(), param).with_knob_size(knob_width));
    let value = ui.plugin_gui_context::<E>().params[param].get();
    ui.label(format!(
        "{}: {}",
        param.name(),
        param.value_to_string(value).unwrap_or_else(|_| String::from("<error>"))
    ));
}

/// Creates a discrete parameter knob
///
/// Displays a dropdown UI element for discrete parameters.
///
/// # Type Parameters
///
/// * `E` - Parameter ID enum that implements `ParamId`
///
/// # Parameters
///
/// * `ui` - The egui UI to render into
/// * `param` - The parameter to display
/// * `num_values` - Number of discrete values for this parameter
fn discrete_knob<E: ParamId>(ui: &mut Ui, param: E, num_values: usize) {
    let valuef = ui.plugin_gui_context::<E>().params[param].get();
    let mut value = valuef.round() as usize;
    let size = ui.available_size_before_wrap();
    let response =
        ComboBox::new(param.name(), "")
            .width(size.x)
            .truncate()
            .show_index(ui, &mut value, num_values, |i| {
                param
                    .value_to_string(i as _)
                    .unwrap_or_else(|_| String::from("<error>"))
            });

    let gui_context = ui.plugin_gui_context::<E>();
    if response.drag_started() {
        gui_context.notifier.notify(ParamChangeEvent {
            id: param,
            kind: ParamChangeKind::GestureBegin,
        });
    }
    if response.changed() {
        gui_context.params[param].set(value as _);
        gui_context.notifier.notify(ParamChangeEvent {
            id: param,
            kind: ParamChangeKind::ValueChange(value as _),
        });
    }
    if response.drag_stopped() {
        gui_context.notifier.notify(ParamChangeEvent {
            id: param,
            kind: ParamChangeKind::GestureEnd,
        });
    }
}
