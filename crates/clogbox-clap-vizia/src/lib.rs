use clogbox_clap::gui::clap_gui::GuiSize;
use clogbox_clap::gui::{GuiEvent, HasRawWindowHandle, PluginView, PluginViewHandle};
use clogbox_clap::main_thread::Plugin;
use clogbox_clap::notifier::Notifier;
use clogbox_clap::params::{ParamChangeEvent, ParamChangeKind, ParamStorage};
use clogbox_clap::processor::{HostSharedHandle, PluginError};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use vizia::prelude::*;
use vizia_baseview::{Application, WindowHandle, WindowScalePolicy};

pub mod components;
pub mod data;
pub mod generic_ui;

pub use generic_ui::generic_ui;

#[derive(Lens)]
pub struct GuiContext<P: Plugin> {
    __plugin: PhantomData<fn() -> P>,
    pub shared_data: P::SharedData,
    pub params: ParamStorage<P::Params>,
    notifier: Notifier<ParamChangeEvent<P::Params>>,
}

impl<P: Plugin> GuiContext<P> {
    pub fn listen_to_param(&mut self, param: P::Params, func: impl 'static + Send + Sync + Fn(f32)) {
        self.notifier.add_listener(move |event| {
            if event.id != param {
                return;
            }
            let ParamChangeKind::ValueChange(value) = event.kind else {
                return;
            };
            func(value);
        })
    }
}

impl<P: Plugin> Clone for GuiContext<P> {
    fn clone(&self) -> Self {
        Self {
            __plugin: PhantomData,
            shared_data: self.shared_data.clone(),
            params: self.params.clone(),
            notifier: self.notifier.clone(),
        }
    }
}

impl<P: Plugin> Model for GuiContext<P> {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.take(|ev: &GuiEvent, _meta| {
            match ev {
                GuiEvent::Resize(_) => {
                    // TODO: figure out resize
                }
                GuiEvent::SetTitle(new_title) => {
                    let Some(window) = cx.window() else {
                        return;
                    };
                    window.set_title(new_title);
                }
                GuiEvent::SetScale(_) => {
                    // TODO: Set scale factor
                }
            }
        });
        event.take(|ev: &ParamChangeEvent<P::Params>, _| {
            if let &ParamChangeKind::ValueChange(value) = &ev.kind {
                self.params.set(ev.id, value);
            }
            self.notifier.notify(ev.clone());
        });
    }
}

struct GuiSizeProxy {
    init: AtomicBool,
    width: AtomicU32,
    height: AtomicU32,
}

impl Default for GuiSizeProxy {
    fn default() -> Self {
        Self {
            init: AtomicBool::new(false),
            width: AtomicU32::new(0),
            height: AtomicU32::new(0),
        }
    }
}

impl GuiSizeProxy {
    fn as_gui_size(&self) -> Option<GuiSize> {
        log::debug!("[GuiSizeProxy] as_gui_size");
        self.init.load(Ordering::Relaxed).then(|| GuiSize {
            width: self.width.load(Ordering::Relaxed),
            height: self.height.load(Ordering::Relaxed),
        })
    }
}

struct SizeProxyModel(Arc<GuiSizeProxy>);

impl Model for SizeProxyModel {
    fn build(self, cx: &mut Context) {
        let current_inner_window_size = EventContext::new(cx).cache.get_bounds(Entity::root());
        let scale = cx.scale_factor();
        let width = current_inner_window_size.width() as f32 / scale;
        let height = current_inner_window_size.height() as f32 / scale;
        self.0.init.store(true, Ordering::Relaxed);
        self.0.width.store(width as u32, Ordering::Relaxed);
        self.0.height.store(height as u32, Ordering::Relaxed);
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|ev: &WindowEvent, _| {
            let Some(window) = cx.window() else {
                return;
            };
            match ev {
                WindowEvent::GeometryChanged(geo)
                    if geo.contains(GeoChanged::WIDTH_CHANGED | GeoChanged::HEIGHT_CHANGED) =>
                {
                    let size = window.inner_size();
                    log::debug!("[WindowEvent] Geometry changed: {:?}", size);
                    self.0.init.store(true, Ordering::Relaxed);
                    self.0.width.store(size.width, Ordering::Relaxed);
                    self.0.height.store(size.height, Ordering::Relaxed);
                }
                _ => {}
            }
        });
    }
}

pub struct ViziaHandle<P: Plugin> {
    __plugin: PhantomData<P>,
    size_proxy: Arc<GuiSizeProxy>,
    events_tx: mpsc::Sender<GuiEvent>,
    handle: WindowHandle,
}

impl<P: Plugin> ViziaHandle<P> {
    fn create(
        window: &dyn HasRawWindowHandle,
        gui_context: GuiContext<P>,
        size: GuiSize,
        view_builder: impl 'static + Send + Fn(&mut Context),
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let rx = Arc::new(Mutex::new(Some(rx)));
        let size_proxy = Arc::<GuiSizeProxy>::default();
        let handle = Application::new({
            let size_proxy = size_proxy.clone();
            move |cx| {
                log::debug!("[vizia::Application] build");
                gui_context.clone().build(cx);
                SizeProxyModel(size_proxy.clone()).build(cx);

                let rx = rx.lock().unwrap().take().unwrap();
                cx.spawn(move |cx| {
                    for event in rx.into_iter() {
                        if let Err(err) = cx.emit(event) {
                            log::error!("Error sending event: {}", err);
                        }
                    }
                });
                view_builder(cx);
            }
        })
        .with_scale_policy(WindowScalePolicy::SystemScaleFactor)
        .inner_size((size.width, size.height))
        .open_parented(&window);
        Self {
            __plugin: PhantomData,
            events_tx: tx,
            size_proxy,
            handle,
        }
    }
}

impl<P: Plugin> PluginViewHandle for ViziaHandle<P> {
    type Params = P::Params;

    fn get_size(&self) -> Option<GuiSize> {
        self.size_proxy.as_gui_size()
    }

    fn send_event(&self, event: GuiEvent) -> Result<(), PluginError> {
        Ok(self.events_tx.send(event)?)
    }
}

pub fn view<P: Plugin>(
    size: GuiSize,
    view: impl 'static + Send + Sync + Fn(&mut Context),
) -> Result<Box<dyn PluginView<Params = P::Params, SharedData = P::SharedData>>, PluginError> {
    struct ViziaView<P> {
        __plugin: PhantomData<P>,
        view: Arc<dyn 'static + Send + Sync + Fn(&mut Context)>,
        size: GuiSize,
    }

    impl<P: Plugin> PluginView for ViziaView<P> {
        type Params = P::Params;
        type SharedData = P::SharedData;

        fn create(
            &mut self,
            window: &dyn HasRawWindowHandle,
            _: HostSharedHandle,
            context: clogbox_clap::gui::GuiContext<Self::Params>,
            shared_data: &Self::SharedData,
        ) -> Result<Box<dyn PluginViewHandle<Params = Self::Params>>, PluginError> {
            log::debug!("[ViziaView] create");
            let gui_context = GuiContext::<P> {
                __plugin: PhantomData,
                shared_data: shared_data.clone(),
                params: context.params,
                notifier: context.notifier,
            };
            let handle = ViziaHandle::create(window, gui_context, self.size, {
                let view = self.view.clone();
                move |cx| {
                    view(cx);
                }
            });
            Ok(Box::new(handle))
        }
    }

    Ok(Box::new(ViziaView::<P> {
        __plugin: PhantomData,
        size,
        view: Arc::new(view),
    }))
}
