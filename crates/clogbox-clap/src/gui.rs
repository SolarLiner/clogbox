use crate::main_thread::Plugin;
use crate::params::{ParamId, ParamNotifier, ParamStorage};
use crate::shared::Shared;
pub use clack_extensions::gui as clap_gui;
use clack_extensions::gui::{GuiSize, PluginGuiImpl, Window};
use clack_plugin::plugin::PluginError;
use clack_plugin::prelude::HostSharedHandle;
use std::marker::PhantomData;

pub use raw_window_handle::HasRawWindowHandle;

pub enum GuiEvent<E> {
    ParamChange(E),
    Resize(GuiSize),
    SetTitle(String),
    SetScale(f64),
}

#[derive(Clone)]
pub struct GuiContext<E: ParamId> {
    pub params: ParamStorage<E>,
    pub dsp_notifier: ParamNotifier<E>,
}

pub trait PluginViewHandle {
    type Params: ParamId;
    fn get_size(&self) -> Option<GuiSize>;
    fn send_event(&self, event: GuiEvent<Self::Params>) -> Result<(), PluginError>;
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
    };

    pub fn notify_param_change(&self, param: P::Params) {
        if let Some(instance) = &self.handle {
            let _ = instance.send_event(GuiEvent::ParamChange(param));
        }
    }

    fn create_instance(
        &mut self,
        host_shared_handle: HostSharedHandle,
        window: Window,
        shared: Shared<P>,
        tx_dsp: ParamNotifier<P::Params>,
    ) -> Result<(), PluginError> {
        eprintln!("Creating GUI instance");
        let context = GuiContext {
            params: shared.params.clone(),
            dsp_notifier: tx_dsp,
        };
        self.handle = Some(self.view.as_mut().unwrap().create(
            &window,
            host_shared_handle,
            context,
            &shared.user_data,
        )?);
        Ok(())
    }
}

macro_rules! delegate_gui_method {
    ($self:ident : $name:ident ($($arg:expr),*); $error:expr) => {{
        let Some(handle) = $self.gui.handle.as_mut() else {
            return $error;
        };
        eprintln!("Calling GUI method: {}(...)", stringify!($name));
        handle.$name($($arg),*)
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
        if self.gui.view.is_none() {
            self.gui.view = Some(self.plugin.view()?);
        }
        Ok(())
    }

    fn destroy(&mut self) {
        eprintln!("[destroy]");
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
        match delegate_gui_method!(self: send_event(GuiEvent::SetTitle(title.to_string())); ()) {
            Ok(()) => {}
            Err(err) => eprintln!("Error setting title: {err}"),
        }
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
