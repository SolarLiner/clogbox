use crate::main_thread::Plugin;
use crate::notifier::Notifier;
use crate::params::{ParamChangeEvent, ParamId, ParamNotifier, ParamStorage};
use crate::shared;
use crate::shared::Shared;
pub use clack_extensions::gui as clap_gui;
use clack_extensions::gui::{GuiSize, PluginGuiImpl, Window};
use clack_plugin::plugin::PluginError;
use clack_plugin::prelude::HostSharedHandle;
pub use raw_window_handle::HasRawWindowHandle;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

pub enum GuiEvent {
    Resize(GuiSize),
    SetTitle(String),
    SetScale(f64),
}

#[derive(Clone)]
pub struct GuiContext<E: ParamId> {
    pub params: ParamStorage<E>,
    pub notifier: Notifier<ParamChangeEvent<E>>,
    sample_rate: Arc<AtomicU64>,
}

impl<E: ParamId> GuiContext<E> {
    pub fn sample_rate(&self) -> f64 {
        let u = self.sample_rate.load(std::sync::atomic::Ordering::Relaxed);
        f64::from_bits(u)
    }
}

pub trait PluginViewHandle {
    type Params: ParamId;
    #[allow(unused_variables)]
    fn load(&mut self, data: serde_json::Value) -> Result<(), PluginError> {
        Ok(())
    }
    fn save(&mut self) -> Result<Option<serde_json::Value>, PluginError> {
        Ok(None)
    }
    fn get_parent(&self) -> Option<Window> {
        None
    }
    fn get_size(&self) -> Option<GuiSize>;
    fn send_event(&self, event: GuiEvent) -> Result<(), PluginError>;
}

pub trait PluginView {
    type Params: ParamId;
    type SharedData;
    fn create(
        &mut self,
        window: &dyn HasRawWindowHandle,
        host: HostSharedHandle,
        context: GuiContext<Self::Params>,
        shared_data: &Self::SharedData,
    ) -> Result<Box<dyn PluginViewHandle<Params = Self::Params>>, PluginError>;
}

pub struct GuiHandle<P: Plugin> {
    __plugin: PhantomData<P>,
    view: Option<Box<dyn PluginView<Params = P::Params, SharedData = P::SharedData>>>,
    handle: Option<Box<dyn PluginViewHandle<Params = P::Params>>>,
    load_data: Option<serde_json::Value>,
}

impl<P: Plugin> Default for GuiHandle<P> {
    fn default() -> Self {
        Self::CONST_DEFAULT
    }
}

impl<P: Plugin> GuiHandle<P> {
    pub const CONST_DEFAULT: Self = Self {
        __plugin: PhantomData,
        handle: None,
        view: None,
        load_data: None,
    };

    pub fn load(&mut self, data: serde_json::Value) -> Result<(), PluginError> {
        if let Some(view) = self.handle.as_deref_mut() {
            view.load(data)?;
        } else {
            self.load_data = Some(data);
        }
        Ok(())
    }

    pub fn save(&mut self) -> Result<Option<serde_json::Value>, PluginError> {
        let Some(instance) = self.handle.as_deref_mut() else {
            return Ok(None);
        };
        instance.save()
    }

    fn create_instance(
        &mut self,
        host_shared_handle: HostSharedHandle,
        window: Window,
        shared: Shared<P>,
    ) -> Result<(), PluginError> {
        log::debug!("Creating GUI instance");
        let context = GuiContext {
            params: shared.params.clone(),
            notifier: shared.notifier.clone(),
            sample_rate: shared.sample_rate.clone(),
        };
        let mut handle = self
            .view
            .as_mut()
            .unwrap()
            .create(&window, host_shared_handle, context, &shared.user_data)?;
        if let Some(data) = self.load_data.take() {
            handle.load(data)?;
        }
        self.handle = Some(handle);
        Ok(())
    }
}

macro_rules! delegate_gui_method {
    ($self:ident : $name:ident ($($arg:expr),*); $error:expr) => {{
        let Some(handle) = $self.gui.handle.as_mut() else {
            return $error;
        };
        log::debug!("Calling GUI method: {}(...)", stringify!($name));
        handle.$name($($arg),*)
    }};
}

impl<P: Plugin> PluginGuiImpl for super::main_thread::MainThread<'_, P> {
    fn is_api_supported(&mut self, gui_config: clap_gui::GuiConfiguration) -> bool {
        log::debug!("[is_api_supported] {gui_config:?}");
        self.get_preferred_api().map_or(false, |api| api == gui_config)
    }

    fn get_preferred_api(&mut self) -> Option<clap_gui::GuiConfiguration> {
        log::debug!("[get_preferred_api]");
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
        log::debug!("[create]");
        if self.gui.view.is_none() {
            self.gui.view = Some(self.plugin.view()?);
        }
        Ok(())
    }

    fn destroy(&mut self) {
        log::debug!("[destroy]");
        self.gui.view.take();
        self.gui.handle.take();
    }

    fn set_scale(&mut self, scale: f64) -> Result<(), PluginError> {
        delegate_gui_method!(self: send_event(GuiEvent::SetScale(scale)); Err(PluginError::Message("GUI instance not created")))
    }

    fn get_size(&mut self) -> Option<GuiSize> {
        delegate_gui_method!(self : get_size(); None)
    }

    fn set_size(&mut self, size: GuiSize) -> Result<(), PluginError> {
        delegate_gui_method!(self : send_event(GuiEvent::Resize(size)); Err(PluginError::Message("GUI instance not created")))
    }

    fn set_parent(&mut self, window: Window) -> Result<(), PluginError> {
        log::debug!("[set_parent] <window>");
        self.gui
            .create_instance(self.host.shared(), window, self.shared.clone())
    }

    fn set_transient(&mut self, window: Window) -> Result<(), PluginError> {
        log::debug!("[set_transient] <window>");
        self.gui
            .create_instance(self.host.shared(), window, self.shared.clone())
    }

    fn suggest_title(&mut self, title: &str) {
        log::debug!("[suggest_title] {title}");
        match delegate_gui_method!(self: send_event(GuiEvent::SetTitle(title.to_string())); ()) {
            Ok(()) => {}
            Err(err) => eprintln!("Error setting title: {err}"),
        }
    }

    fn show(&mut self) -> Result<(), PluginError> {
        log::debug!("[show]");
        Err(PluginError::Message("Not implemented"))
    }

    fn hide(&mut self) -> Result<(), PluginError> {
        log::debug!("[hide]");
        Err(PluginError::Message("Not implemented"))
    }
}
