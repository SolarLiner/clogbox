use crate::main_thread::Plugin;
use crate::params::{ParamChangeEvent, ParamChangeKind, ParamId, ParamNotifier};
use crate::shared::Shared;
use baseview::{PhySize, Size, WindowScalePolicy};
pub use clack_extensions::gui as clap_gui;
use clack_extensions::gui::{GuiSize, PluginGuiImpl, Window};
use clack_plugin::plugin::PluginError;
use clack_plugin::prelude::HostSharedHandle;
use clogbox_enum::enum_iter;
use egui::{widgets, Widget};
use raw_window_handle::HasRawWindowHandle;
use ringbuf::traits::Producer;
use std::cmp::min;
use std::sync::mpsc::{Receiver, Sender};

pub enum GuiEvent<E> {
    ParamChange(E),
    Resize(GuiSize),
    SetTitle(String),
}

pub struct View<E: ParamId> {
    handle: baseview::WindowHandle,
    shared: Shared<E>,
    size: GuiSize,
    pub tx: Sender<GuiEvent<E>>,
}

impl<E: ParamId> View<E> {
    fn new(
        host_shared_handle: HostSharedHandle,
        window: &impl HasRawWindowHandle,
        dsp_notifier: ParamNotifier<E>,
        shared: Shared<E>,
    ) -> Self {
        struct GuiState<E: ParamId> {
            host_gui: clack_extensions::gui::HostGui,
            params: clack_extensions::params::HostParams,
            shared: Shared<E>,
            rx: Receiver<GuiEvent<E>>,
            dsp_notifier: ParamNotifier<E>,
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let size = GuiSize {
            width: 800,
            height: 350,
        };
        let state = GuiState {
            host_gui: host_shared_handle.get_extension().unwrap(),
            params: host_shared_handle.get_extension().unwrap(),
            rx,
            dsp_notifier,
            shared: shared.clone(),
        };
        let open_options = baseview::WindowOpenOptions {
            title: "egui plugin window".into(),
            size: Size::new(size.width as f64, size.height as f64),
            scale: WindowScalePolicy::SystemScaleFactor,
            gl_config: None,
        };
        let graphics_config = egui_baseview::GraphicsConfig::default();

        let handle = egui_baseview::EguiWindow::open_parented(
            window,
            open_options,
            graphics_config,
            state,
            |_, _, _| (),
            |ctx, queue, state| {
                for event in state.rx.try_iter() {
                    match event {
                        GuiEvent::Resize(size) => {
                            queue.resize(PhySize {
                                width: size.width,
                                height: size.height,
                            });
                        }
                        GuiEvent::ParamChange(..) => {
                            ctx.request_repaint();
                        }
                        _ => {}
                    }
                }
                if enum_iter::<E>().any(|p| state.shared.params[p].has_changed()) {
                    ctx.request_repaint();
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::Grid::new("params")
                        .num_columns(2)
                        .spacing([40.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for p in enum_iter::<E>() {
                                let mut value = state.shared.params.get_normalized(p);
                                ui.label(p.name());
                                let response = widgets::DragValue::new(&mut value)
                                    .custom_parser(|s| p.text_to_value(s).map(|f| p.mapping().normalize(f) as _))
                                    .custom_formatter(|f, _| {
                                        p.value_to_string(p.mapping().denormalize(f as _))
                                            .unwrap_or_else(|err| format!("Formatting error: {err}"))
                                    })
                                    .speed(0.005)
                                    .range(0.0..=1.0)
                                    .ui(ui);
                                ui.end_row();
                                if response.changed() {
                                    state.shared.params.set(p, p.mapping().denormalize(value as _));
                                    state
                                        .dsp_notifier
                                        .notify(p, ParamChangeKind::ValueChange(p.mapping().denormalize(value as _)));
                                }
                            }
                        });
                });
                // egui::Window::new("debug settings")
                //     .collapsible(true)
                //     .vscroll(true)
                //     .show(ctx, |ui| {
                //         ctx.settings_ui(ui);
                //     });
            },
        );
        Self {
            handle,
            size,
            shared,
            tx,
        }
    }

    fn set_scale(&mut self, _scale: f64) -> Result<(), PluginError> {
        Ok(())
    }

    fn get_size(&self) -> Option<GuiSize> {
        Some(self.size)
    }

    fn set_size(&mut self, size: GuiSize) -> Result<(), PluginError> {
        self.size = size;
        self.send_event(GuiEvent::Resize(size))
    }

    fn send_event(&self, event: GuiEvent<E>) -> Result<(), PluginError> {
        self.tx.send(event).map_err(|err| PluginError::Error(Box::new(err)))
    }

    fn set_title(&mut self, title: &str) {
        let _ = self.send_event(GuiEvent::SetTitle(title.to_string()));
    }
}

pub struct GuiHandle<E: ParamId> {
    instance: Option<View<E>>,
}

impl<E: ParamId> Default for GuiHandle<E> {
    fn default() -> Self {
        Self::CONST_DEFAULT
    }
}

impl<E: ParamId> GuiHandle<E> {
    pub const CONST_DEFAULT: Self = Self { instance: None };

    pub fn notify_param_change(&self, param: E) {
        if let Some(instance) = &self.instance {
            let _ = instance.send_event(GuiEvent::ParamChange(param));
        }
    }

    fn create_instance(
        &mut self,
        host_shared_handle: HostSharedHandle,
        window: Window,
        shared: Shared<E>,
        tx_dsp: ParamNotifier<E>,
    ) -> Result<(), PluginError> {
        eprintln!("Creating GUI instance");
        self.instance = Some(View::new(host_shared_handle, &window, tx_dsp, shared));
        Ok(())
    }
}

macro_rules! delegate_gui_method {
    ($self:ident : $name:ident ($($arg:expr),*); $error:expr) => {{
        let Some(instance) = $self.gui.instance.as_mut() else {
            return $error;
        };
        eprintln!("Calling GUI method: {}(...)", stringify!($name));
        instance.$name($($arg),*)
    }};
}

impl<P: Plugin> PluginGuiImpl for super::main_thread::MainThread<'_, P> {
    fn is_api_supported(&mut self, gui_config: clap_gui::GuiConfiguration) -> bool {
        eprintln!("[is_api_supported] {gui_config:?}");
        self.get_preferred_api().map_or(false, |api| api == gui_config)
    }

    fn get_preferred_api(&mut self) -> Option<clap_gui::GuiConfiguration> {
        eprintln!("[get_preferred_api]");
        if cfg!(target_os = "macos") {
            Some(clap_gui::GuiConfiguration {
                api_type: clap_gui::GuiApiType::COCOA,
                is_floating: false,
            })
        } else if cfg!(target_os = "windows") {
            Some(clap_gui::GuiConfiguration {
                api_type: clap_gui::GuiApiType::WIN32,
                is_floating: false,
            })
        } else if cfg!(target_os = "linux") {
            Some(clap_gui::GuiConfiguration {
                api_type: clap_gui::GuiApiType::X11,
                is_floating: false,
            })
        } else {
            None
        }
    }

    fn create(&mut self, _configuration: clap_gui::GuiConfiguration) -> Result<(), PluginError> {
        eprintln!("[create]");
        Ok(())
    }

    fn destroy(&mut self) {
        eprintln!("[destroy]");
        self.gui.instance.take();
    }

    fn set_scale(&mut self, scale: f64) -> Result<(), PluginError> {
        eprintln!("[set_scale] {scale}");
        delegate_gui_method!(self: set_scale(scale); Err(PluginError::Message("GUI instance not created")))
    }

    fn get_size(&mut self) -> Option<GuiSize> {
        eprintln!("[get_size]");
        delegate_gui_method!(self : get_size(); None)
    }

    fn set_size(&mut self, size: GuiSize) -> Result<(), PluginError> {
        eprintln!("[set_size] {size:?}");
        delegate_gui_method!(self : set_size(size); Err(PluginError::Message("GUI instance not created")))
    }

    fn set_parent(&mut self, window: Window) -> Result<(), PluginError> {
        eprintln!("[set_parent] <window>");
        self.gui.create_instance(
            self.host.shared(),
            window,
            self.shared.clone(),
            self.param_notifier.clone(),
        )
    }

    fn set_transient(&mut self, window: Window) -> Result<(), PluginError> {
        eprintln!("[set_transient] <window>");
        self.gui.create_instance(
            self.host.shared(),
            window,
            self.shared.clone(),
            self.param_notifier.clone(),
        )
    }

    fn suggest_title(&mut self, title: &str) {
        eprintln!("[suggest_title] {title}");
        delegate_gui_method!(self: set_title(title); ())
    }

    fn show(&mut self) -> Result<(), PluginError> {
        eprintln!("[show]");
        Err(PluginError::Message("Not implemented"))
    }

    fn hide(&mut self) -> Result<(), PluginError> {
        eprintln!("[hide]");
        Err(PluginError::Message("Not implemented"))
    }
}
