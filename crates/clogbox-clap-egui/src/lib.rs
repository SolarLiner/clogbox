use baseview::{PhySize, Size, WindowHandle, WindowScalePolicy};
use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::{GuiContext, GuiEvent, HasRawWindowHandle, PluginView, PluginViewHandle};
use clogbox_clap::params::ParamId;
use clogbox_clap::processor::{HostSharedHandle, PluginError};
use egui::Id;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU32;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

pub use egui;
pub use egui_baseview;

pub mod components;
pub mod generic_ui;

pub use generic_ui::generic_ui;

pub fn gui_context_id() -> Id {
    Id::new("gui_context")
}

pub fn shared_data_id() -> Id {
    Id::new("shared_data")
}

pub trait EguiPluginView: 'static + Send {
    type Params: ParamId;

    #[allow(unused_variables)]
    fn build(&mut self, ctx: &egui::Context, queue: &mut egui_baseview::Queue) {}
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

pub struct EguiHandle<E: ParamId, SharedData> {
    __paramid: PhantomData<E>,
    __shared_data: PhantomData<SharedData>,
    tx: Sender<GuiEvent<E>>,
    current_size: ArcSize,
    pub handle: WindowHandle,
}

impl<E: ParamId, SharedData> PluginViewHandle for EguiHandle<E, SharedData> {
    type Params = E;
    fn get_size(&self) -> Option<GuiSize> {
        Some(self.current_size.get())
    }

    fn send_event(&self, event: GuiEvent<E>) -> Result<(), PluginError> {
        self.tx.send(event).map_err(|err| PluginError::Error(Box::new(err)))
    }
}

impl<E: ParamId, SharedData: 'static + Send + Sync + Clone> EguiHandle<E, SharedData> {
    fn create(
        window: &dyn HasRawWindowHandle,
        context: GuiContext<E>,
        instance: Box<dyn EguiPluginView<Params = E>>,
        size: GuiSize,
        shared_data: &SharedData,
    ) -> Result<Self, PluginError> {
        struct GuiState<E: ParamId> {
            current_size: ArcSize,
            rx: Receiver<GuiEvent<E>>,
            instance: Box<dyn EguiPluginView<Params = E>>,
        }

        let mut context = Some(context);
        let mut shared_data = Some(shared_data.clone());
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
            move |ctx, queue, state| {
                let GuiState { instance, .. } = state;
                ctx.data_mut(|data| {
                    data.insert_temp(gui_context_id(), context.take().unwrap());
                    data.insert_temp(shared_data_id(), shared_data.take().unwrap());
                });
                instance.build(ctx, queue);
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
                        GuiEvent::ParamChange(_) => ctx.request_repaint(),
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
        })
    }
}

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
