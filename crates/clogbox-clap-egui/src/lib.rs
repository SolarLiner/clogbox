//! # `egui` for the `clogbox` CLAP wrapper
//!
//! Integration of the `egui` immediate mode GUI library with the `clogbox` CLAP plugin wrapper.
//!
//! This crate provides utilities for creating GUI interfaces for CLAP audio plugins using the
//! `egui` library. It includes components for parameter visualization and manipulation, as well
//! as a generic UI that can be used to quickly create interfaces for plugins.

#![warn(missing_docs)]

use baseview::{PhySize, Size, WindowHandle, WindowScalePolicy};
use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::{GuiContext, GuiEvent, HasRawWindowHandle, PluginView, PluginViewHandle};
use clogbox_clap::params::ParamId;
use clogbox_clap::{HostSharedHandle, PluginError};
use egui::Id;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU32;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use clogbox_clap::Plugin;
pub use egui;
pub use egui_baseview;
use serde_json::Value;

pub mod components;
pub mod generic_ui;

pub use generic_ui::generic_ui;

fn gui_context_id() -> Id {
    Id::new("gui_context")
}

fn shared_data_id() -> Id {
    Id::new("shared_data")
}

/// Trait for implementing `egui`-based plugin views.
///
/// This trait defines the interface for creating custom `egui`-based views for CLAP plugins.
/// Implementors of this trait can define how the plugin's GUI is built and updated.
pub trait EguiPluginView: 'static + Send {
    /// The parameter ID type used by this view.
    type Params: ParamId;

    /// Called once when the view is first created.
    ///
    /// This method can be used to set up the initial state of the view.
    /// The default implementation does nothing.
    #[allow(unused_variables)]
    fn build(&mut self, ctx: &egui::Context, queue: &mut egui_baseview::Queue) {}

    /// Called on each frame to update the view.
    ///
    /// This method should implement the actual GUI rendering logic.
    fn update(&mut self, ctx: &egui::Context, queue: &mut egui_baseview::Queue);
}

struct ArcSizeInner {
    width: AtomicU32,
    height: AtomicU32,
}

impl ArcSizeInner {
    const ORDERING: std::sync::atomic::Ordering = std::sync::atomic::Ordering::Relaxed;
    fn new(width: u32, height: u32) -> Arc<Self> {
        Arc::new(Self {
            width: AtomicU32::new(width),
            height: AtomicU32::new(height),
        })
    }

    fn get(&self) -> GuiSize {
        GuiSize {
            width: self.width.load(Self::ORDERING),
            height: self.height.load(Self::ORDERING),
        }
    }

    fn set(&self, width: u32, height: u32) {
        self.width.store(width, Self::ORDERING);
        self.height.store(height, Self::ORDERING);
    }
}

type ArcSize = Arc<ArcSizeInner>;

fn arc_size(width: u32, height: u32) -> ArcSize {
    ArcSizeInner::new(width, height)
}

/// Handle for an `egui`-based plugin view.
///
/// This struct represents a handle to an `egui`-based plugin view window.
/// It implements the `PluginViewHandle` trait and provides methods for
/// interacting with the view, such as loading and saving state, getting
/// the current size, and sending events.
pub struct EguiHandle<E: ParamId, SharedData> {
    __paramid: PhantomData<E>,
    __shared_data: PhantomData<SharedData>,
    /// The underlying window handle.
    pub handle: WindowHandle,
    tx: Sender<GuiEvent>,
    current_size: ArcSize,
    egui_context: egui::Context,
}

impl<E: ParamId, SharedData> PluginViewHandle for EguiHandle<E, SharedData> {
    type Params = E;
    fn load(&mut self, data: Value) -> Result<(), PluginError> {
        self.egui_context.memory_mut(|mem| {
            *mem = serde_json::from_value(data)?;
            Ok(())
        })
    }
    fn save(&mut self) -> Result<Option<Value>, PluginError> {
        self.egui_context.memory(|mem| {
            Ok(serde_json::to_value(mem).map(Some).unwrap_or_else(|err| {
                log::error!("Failed to serialize egui memory: {}", err);
                None
            }))
        })
    }

    fn get_size(&self) -> Option<GuiSize> {
        Some(self.current_size.get())
    }

    fn send_event(&self, event: GuiEvent) -> Result<(), PluginError> {
        log::info!("Sending event: request repaint");
        self.egui_context.request_repaint();
        self.tx.send(event).map_err(|err| PluginError::Error(Box::new(err)))
    }
}

impl<E: ParamId, SharedData: 'static + Send + Sync + Clone> EguiHandle<E, SharedData> {
    //noinspection RsUnwrap
    fn create(
        window: &dyn HasRawWindowHandle,
        context: GuiContext<E>,
        instance: Box<dyn EguiPluginView<Params = E>>,
        size: GuiSize,
        shared_data: &SharedData,
    ) -> Result<Self, PluginError> {
        struct GuiState<E: ParamId> {
            current_size: ArcSize,
            rx: Receiver<GuiEvent>,
            instance: Box<dyn EguiPluginView<Params = E>>,
        }

        let mut context = Some(context);
        let mut shared_data = Some(shared_data.clone());
        let egui_context = Arc::new(Mutex::new(None));
        let (tx, rx) = std::sync::mpsc::channel();
        let current_size = arc_size(size.width, size.height);
        let state = GuiState {
            rx,
            current_size: current_size.clone(),
            instance,
        };
        let open_options = baseview::WindowOpenOptions {
            title: "egui plugin window".into(),
            size: Size::new(size.width as f64, size.height as f64),
            scale: WindowScalePolicy::SystemScaleFactor,
            gl_config: None,
        };
        let graphics_config = egui_baseview::GraphicsConfig::default();

        let handle = egui_baseview::EguiWindow::open_parented(
            &window,
            open_options,
            graphics_config,
            state,
            {
                let egui_context = egui_context.clone();
                move |ctx, queue, state| {
                    let GuiState { instance, .. } = state;
                    let context = context.take().unwrap();
                    context.notifier.add_listener({
                        let ctx = ctx.clone();
                        move |event| {
                            ctx.request_repaint();
                        }
                    });

                    ctx.data_mut(|data| {
                        data.insert_temp(gui_context_id(), context);
                        data.insert_temp(shared_data_id(), shared_data.take().unwrap());
                    });
                    egui_context.lock().unwrap().replace(ctx.clone());
                    instance.build(ctx, queue);
                }
            },
            |ctx, queue, state| {
                for event in state.rx.try_iter() {
                    match event {
                        GuiEvent::Resize(size) => {
                            queue.resize(PhySize {
                                width: size.width,
                                height: size.height,
                            });
                            state.current_size.set(size.width, size.height);
                        }
                        _ => {}
                    }
                }
                state.instance.update(ctx, queue);
            },
        );
        Ok(Self {
            __paramid: PhantomData,
            __shared_data: PhantomData,
            handle,
            tx,
            current_size,
            egui_context: {
                let mut guard = egui_context.lock().unwrap();
                guard.take().unwrap()
            },
        })
    }
}

/// Creates a plugin view from an `egui` view implementation.
///
/// This function takes an implementation of the `EguiPluginView` trait and wraps it
/// in a `PluginView` implementation that can be used with the CLAP plugin framework.
///
/// # Type Parameters
///
/// * `E` - The parameter ID type used by the view
/// * `SharedData` - The type of shared data used by the plugin
///
/// # Parameters
///
/// * `size` - The initial size of the view window
/// * `view` - The `egui` view implementation
///
/// # Returns
///
/// A boxed `PluginView` implementation or an error
pub fn view<E: ParamId, SharedData: 'static + Send + Sync + Clone>(
    size: GuiSize,
    view: impl EguiPluginView<Params = E>,
) -> Result<Box<dyn PluginView<Params = E, SharedData = SharedData>>, PluginError> {
    struct Impl<V: EguiPluginView, SharedData>(GuiSize, Option<V>, PhantomData<SharedData>);

    impl<V: EguiPluginView, SharedData: 'static + Send + Sync + Clone> PluginView for Impl<V, SharedData> {
        type Params = V::Params;
        type SharedData = SharedData;

        fn create(
            &mut self,
            window: &dyn HasRawWindowHandle,
            _host: HostSharedHandle,
            context: GuiContext<Self::Params>,
            shared_data: &Self::SharedData,
        ) -> Result<Box<dyn PluginViewHandle<Params = Self::Params>>, PluginError> {
            let handle = EguiHandle::create(window, context, Box::new(self.1.take().unwrap()), self.0, shared_data)?;
            Ok(Box::new(handle))
        }
    }

    Ok(Box::new(Impl(size, Some(view), PhantomData)))
}

/// Extension trait for accessing plugin-specific data from `egui` contexts.
///
/// This trait extends `egui`'s `Context` and `Ui` types with methods for accessing
/// plugin-specific data, such as the GUI context and shared data.
pub trait GetContextExtra {
    /// Retrieves the plugin GUI context from an `egui` context.
    ///
    /// This method provides access to the plugin's parameter state and notification system.
    fn plugin_gui_context<E: ParamId>(&self) -> GuiContext<E>;

    /// Retrieves the plugin's shared data from an `egui` context.
    ///
    /// This method provides access to the plugin's shared state that is accessible
    /// from both the GUI and audio processing threads.
    fn plugin_shared_data<P: Plugin>(&self) -> P::SharedData;
}

impl GetContextExtra for egui::Context {
    fn plugin_gui_context<E: ParamId>(&self) -> GuiContext<E> {
        self.data(|data| data.get_temp(gui_context_id()).unwrap())
    }

    fn plugin_shared_data<P: Plugin>(&self) -> P::SharedData {
        self.data(|data| data.get_temp(shared_data_id()).unwrap())
    }
}

impl GetContextExtra for egui::Ui {
    fn plugin_gui_context<E: ParamId>(&self) -> GuiContext<E> {
        self.ctx().plugin_gui_context::<E>()
    }

    fn plugin_shared_data<P: Plugin>(&self) -> P::SharedData {
        self.ctx().plugin_shared_data::<P>()
    }
}
