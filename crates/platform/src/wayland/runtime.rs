use crate::{
    BackendKind, ControlFlow, CursorIcon, Key, KeyEvent, LogicalPosition, LogicalSize, MouseButton,
    MouseScrollDelta, PhysicalPosition, PhysicalSize, PlatformBackend, PlatformWindow,
    PopupOptions, ResizeDirection, WindowAttributes, WindowEvent, WindowId, WindowLevel,
    WindowRole,
};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};
use smithay_client_toolkit::{
    compositor::{CompositorState, Surface, SurfaceData},
    globals::GlobalData,
    reexports::{
        client::{
            globals::{registry_queue_init, GlobalListContents},
            protocol::{
                wl_compositor, wl_display, wl_keyboard, wl_pointer, wl_region, wl_registry,
                wl_seat, wl_surface,
            },
            Connection, Dispatch, Proxy, QueueHandle, WEnum,
        },
        csd_frame::WindowState,
        protocols::{
            wp::cursor_shape::v1::client::{
                wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1},
                wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
            },
            xdg::shell::client::xdg_surface,
        },
    },
    seat::pointer::cursor_shape::CursorShapeManager,
    shell::{
        xdg::{
            popup::{Popup, PopupConfigure, PopupHandler},
            window::{Window as XdgWindow, WindowConfigure, WindowDecorations, WindowHandler},
            XdgPositioner, XdgShell, XdgSurface,
        },
        WaylandSurface,
    },
};
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    marker::PhantomData,
    os::fd::AsRawFd,
    ptr::NonNull,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

const USER_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(8);

pub trait ApplicationHandler<T: 'static> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop<'_>);
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop<'_>,
        window_id: WindowId,
        event: WindowEvent,
    );
    fn user_event(&mut self, event_loop: &ActiveEventLoop<'_>, event: T);
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop<'_>);
}

#[derive(Default)]
struct LoopState {
    exit: AtomicBool,
    control_flow: Mutex<ControlFlow>,
}

impl Default for ControlFlow {
    fn default() -> Self {
        Self::Wait
    }
}

pub struct EventLoop<T: 'static> {
    events: Arc<Mutex<Vec<T>>>,
    state: Arc<LoopState>,
}

pub struct EventLoopBuilder<T: 'static> {
    marker: PhantomData<T>,
}

pub struct EventLoopProxy<T: 'static> {
    events: Arc<Mutex<Vec<T>>>,
}

impl<T: 'static> EventLoop<T> {
    pub fn with_user_event() -> EventLoopBuilder<T> {
        EventLoopBuilder {
            marker: PhantomData,
        }
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            events: self.events.clone(),
        }
    }

    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        *self
            .state
            .control_flow
            .lock()
            .expect("control flow lock poisoned") = control_flow;
    }

    pub fn run_app<H: ApplicationHandler<T>>(self, handler: &mut H) -> Result<(), String> {
        let connection = Connection::connect_to_env().map_err(|error| error.to_string())?;
        let (globals, mut event_queue) =
            registry_queue_init::<DispatchState>(&connection).map_err(|error| error.to_string())?;
        let queue_handle = event_queue.handle();
        let compositor =
            CompositorState::bind(&globals, &queue_handle).map_err(|error| error.to_string())?;
        let xdg_shell =
            XdgShell::bind(&globals, &queue_handle).map_err(|error| error.to_string())?;
        let cursor_shape_manager = CursorShapeManager::bind(&globals, &queue_handle).ok();
        let layer_shell = globals.bind(&queue_handle, 1..=5, ()).ok();
        let seat = globals.bind(&queue_handle, 1..=9, ()).ok();
        let runtime = Runtime::new(
            compositor,
            xdg_shell,
            connection.display(),
            seat,
            cursor_shape_manager,
            layer_shell,
        );
        let mut dispatch = DispatchState {
            runtime: runtime.clone(),
        };
        let active = ActiveEventLoop {
            runtime: &runtime,
            queue_handle: &queue_handle,
            state: &self.state,
        };
        handler.resumed(&active);

        while !self.state.exit.load(Ordering::Acquire) {
            handler.about_to_wait(&active);
            for event in self
                .events
                .lock()
                .expect("user event queue lock poisoned")
                .drain(..)
            {
                handler.user_event(&active, event);
            }
            runtime.borrow_mut().close_requested();
            runtime.borrow_mut().apply_cursor_requests();
            dispatch_redraws(&runtime);
            let timeout = timeout_for(&self.state);
            dispatch_with_timeout(&connection, &mut event_queue, timeout, &mut dispatch)?;
            for (window_id, event) in runtime.borrow_mut().events.drain(..) {
                handler.window_event(&active, window_id, event);
            }
        }
        Ok(())
    }
}

impl<T: 'static> EventLoopBuilder<T> {
    #[cfg(target_os = "linux")]
    pub fn with_any_thread(&mut self, _any_thread: bool) {}

    pub fn build(self) -> Result<EventLoop<T>, String> {
        Ok(EventLoop {
            events: Arc::new(Mutex::new(Vec::new())),
            state: Arc::new(LoopState::default()),
        })
    }
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
        }
    }
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), String> {
        self.events
            .lock()
            .map_err(|_| "user event queue lock poisoned".to_owned())?
            .push(event);
        Ok(())
    }
}

pub struct ActiveEventLoop<'a> {
    runtime: &'a RcRuntime,
    queue_handle: &'a QueueHandle<DispatchState>,
    state: &'a Arc<LoopState>,
}

impl ActiveEventLoop<'_> {
    pub fn create_window(
        &self,
        attributes: WindowAttributes,
    ) -> Result<Arc<dyn PlatformWindow>, String> {
        let id = WindowId::next();
        let mut runtime = self.runtime.borrow_mut();
        if runtime.layer_shell.is_some()
            && matches!(
                attributes.role,
                WindowRole::Desktop | WindowRole::Overlay | WindowRole::BottomPanel
            )
        {
            return create_layer_window(&mut runtime, self.queue_handle, id, attributes);
        }
        let surface = runtime.compositor.create_surface(self.queue_handle);
        let decorations = if attributes.decorations {
            WindowDecorations::RequestServer
        } else {
            WindowDecorations::None
        };
        let window = runtime
            .xdg_shell
            .create_window(surface, decorations, self.queue_handle);
        window.set_title(attributes.title);
        window.set_min_size(Some((
            attributes.size.width.ceil() as u32,
            attributes.size.height.ceil() as u32,
        )));
        window.commit();
        let handle = Arc::new(Window::new(
            id,
            window.wl_surface().clone(),
            runtime.display.clone(),
            runtime.close_requests.clone(),
            runtime.cursor_requests.clone(),
            attributes.size,
            false,
        ));
        runtime.windows.insert(
            id,
            NativeWindow::Toplevel {
                window,
                handle: handle.clone(),
            },
        );
        Ok(handle)
    }

    pub fn create_popup(
        &self,
        attributes: WindowAttributes,
        popup_options: PopupOptions,
    ) -> Result<Arc<dyn PlatformWindow>, String> {
        let id = WindowId::next();
        let mut runtime = self.runtime.borrow_mut();
        let parent = runtime
            .windows
            .get(&popup_options.parent)
            .ok_or_else(|| "popup parent no longer exists".to_owned())?
            .popup_parent();
        let geometry = super::PopupPositioner::from_popup(&popup_options, attributes.size);
        let grab = popup_options
            .input_serial
            .zip(runtime.seat.as_ref())
            .map(|(serial, seat)| (seat, serial));
        let popup = match parent {
            PopupParent::Xdg(parent) => super::create_popup(
                &parent,
                geometry,
                self.queue_handle,
                &runtime.compositor,
                &runtime.xdg_shell,
                grab,
            )
            .map_err(|error| error.to_string())?,
            PopupParent::Layer(parent) => create_layer_popup(
                &parent,
                geometry,
                self.queue_handle,
                &runtime.compositor,
                &runtime.xdg_shell,
                grab,
            )
            .map_err(|error| error.to_string())?,
        };
        let handle = Arc::new(Window::new(
            id,
            popup.wl_surface().clone(),
            runtime.display.clone(),
            runtime.close_requests.clone(),
            runtime.cursor_requests.clone(),
            attributes.size,
            false,
        ));
        runtime.windows.insert(
            id,
            NativeWindow::Popup {
                popup,
                handle: handle.clone(),
            },
        );
        Ok(handle)
    }

    pub fn exit(&self) {
        self.state.exit.store(true, Ordering::Release);
    }

    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        *self
            .state
            .control_flow
            .lock()
            .expect("control flow lock poisoned") = control_flow;
    }
}

impl PlatformBackend for ActiveEventLoop<'_> {
    fn kind(&self) -> BackendKind {
        BackendKind::Wayland
    }

    fn create_window(
        &self,
        attributes: WindowAttributes,
    ) -> Result<Arc<dyn PlatformWindow>, String> {
        Self::create_window(self, attributes)
    }

    fn create_popup(
        &self,
        attributes: WindowAttributes,
        popup: PopupOptions,
    ) -> Result<Arc<dyn PlatformWindow>, String> {
        Self::create_popup(self, attributes, popup)
    }
}

type RcRuntime = std::rc::Rc<RefCell<Runtime>>;

struct Runtime {
    compositor: CompositorState,
    xdg_shell: XdgShell,
    display: wl_display::WlDisplay,
    seat: Option<wl_seat::WlSeat>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    cursor_shape_manager: Option<CursorShapeManager>,
    layer_shell: Option<ZwlrLayerShellV1>,
    cursor_shape_device: Option<WpCursorShapeDeviceV1>,
    cursor_serial: Option<u32>,
    pointer_focus: Option<WindowId>,
    last_pointer_target: Option<WindowId>,
    pointer_over_passthrough: bool,
    keyboard_focus: Option<WindowId>,
    close_requests: Arc<Mutex<Vec<WindowId>>>,
    cursor_requests: Arc<Mutex<Vec<(WindowId, CursorIcon)>>>,
    windows: HashMap<WindowId, NativeWindow>,
    events: Vec<(WindowId, WindowEvent)>,
}

impl Runtime {
    fn new(
        compositor: CompositorState,
        xdg_shell: XdgShell,
        display: wl_display::WlDisplay,
        seat: Option<wl_seat::WlSeat>,
        cursor_shape_manager: Option<CursorShapeManager>,
        layer_shell: Option<ZwlrLayerShellV1>,
    ) -> RcRuntime {
        std::rc::Rc::new(RefCell::new(Self {
            compositor,
            xdg_shell,
            display,
            seat,
            pointer: None,
            keyboard: None,
            cursor_shape_manager,
            layer_shell,
            cursor_shape_device: None,
            cursor_serial: None,
            pointer_focus: None,
            last_pointer_target: None,
            pointer_over_passthrough: false,
            keyboard_focus: None,
            close_requests: Arc::new(Mutex::new(Vec::new())),
            cursor_requests: Arc::new(Mutex::new(Vec::new())),
            windows: HashMap::new(),
            events: Vec::new(),
        }))
    }

    fn id_for_surface(&self, surface: &wl_surface::WlSurface) -> Option<WindowId> {
        self.windows
            .iter()
            .find_map(|(id, window)| (window.surface() == surface).then_some(*id))
    }

    fn pointer_target(&self) -> Option<WindowId> {
        if self.pointer_over_passthrough {
            self.last_pointer_target
        } else {
            self.pointer_focus
        }
    }

    fn close_requested(&mut self) {
        let requests = std::mem::take(
            &mut *self
                .close_requests
                .lock()
                .expect("window close request lock poisoned"),
        );
        for id in requests {
            self.windows.remove(&id);
            if self.pointer_focus == Some(id) {
                self.pointer_focus = None;
            }
            if self.keyboard_focus == Some(id) {
                self.keyboard_focus = None;
            }
        }
    }

    fn apply_cursor_requests(&mut self) {
        let requests = std::mem::take(
            &mut *self
                .cursor_requests
                .lock()
                .expect("cursor request lock poisoned"),
        );
        let (Some(device), Some(serial), Some(focused)) = (
            self.cursor_shape_device.as_ref(),
            self.cursor_serial,
            self.pointer_focus,
        ) else {
            return;
        };
        for (window_id, icon) in requests {
            if window_id == focused {
                device.set_shape(serial, cursor_shape(icon));
            }
        }
    }
}

fn create_layer_window(
    runtime: &mut Runtime,
    queue_handle: &QueueHandle<DispatchState>,
    id: WindowId,
    attributes: WindowAttributes,
) -> Result<Arc<dyn PlatformWindow>, String> {
    let layer_shell = runtime
        .layer_shell
        .as_ref()
        .ok_or_else(|| "the compositor does not support the layer-shell protocol".to_owned())?;
    let surface = runtime.compositor.create_surface(queue_handle);
    let desktop = attributes.role == WindowRole::Desktop;
    let overlay = attributes.role == WindowRole::Overlay;
    let layer_surface = layer_shell.get_layer_surface(
        &surface,
        None,
        if desktop { Layer::Bottom } else { Layer::Top },
        if desktop {
            "creamshell.desktop".to_owned()
        } else {
            "creamshell".to_owned()
        },
        queue_handle,
        (),
    );
    if desktop || overlay {
        layer_surface.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
        layer_surface.set_size(0, 0);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        if overlay {
            let input_region = runtime
                .compositor
                .wl_compositor()
                .create_region(queue_handle, ());
            input_region.add(0, 0, 0, 0);
            surface.set_input_region(Some(&input_region));
            input_region.destroy();
        }
    } else {
        layer_surface.set_anchor(Anchor::Bottom | Anchor::Left | Anchor::Right);
        layer_surface.set_size(0, attributes.size.height.ceil().max(1.0) as u32);
        layer_surface.set_exclusive_zone(attributes.size.height.ceil().max(1.0) as i32);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    }
    surface.commit();
    let handle = Arc::new(Window::new(
        id,
        surface,
        runtime.display.clone(),
        runtime.close_requests.clone(),
        runtime.cursor_requests.clone(),
        attributes.size,
        overlay,
    ));
    runtime.windows.insert(
        id,
        NativeWindow::Layer {
            layer_surface,
            handle: handle.clone(),
        },
    );
    Ok(handle)
}

enum NativeWindow {
    Toplevel {
        window: XdgWindow,
        handle: Arc<Window>,
    },
    Popup {
        popup: Popup,
        handle: Arc<Window>,
    },
    Layer {
        layer_surface: ZwlrLayerSurfaceV1,
        handle: Arc<Window>,
    },
}

enum PopupParent {
    Xdg(xdg_surface::XdgSurface),
    Layer(ZwlrLayerSurfaceV1),
}

impl NativeWindow {
    fn surface(&self) -> &wl_surface::WlSurface {
        match self {
            Self::Toplevel { window, .. } => window.wl_surface(),
            Self::Popup { popup, .. } => popup.wl_surface(),
            Self::Layer { handle, .. } => &handle.surface,
        }
    }

    fn popup_parent(&self) -> PopupParent {
        match self {
            Self::Toplevel { window, .. } => PopupParent::Xdg(window.xdg_surface().clone()),
            Self::Popup { popup, .. } => PopupParent::Xdg(popup.xdg_surface().clone()),
            Self::Layer { layer_surface, .. } => PopupParent::Layer(layer_surface.clone()),
        }
    }

    fn handle(&self) -> &Arc<Window> {
        match self {
            Self::Toplevel { handle, .. } | Self::Popup { handle, .. } => handle,
            Self::Layer { handle, .. } => handle,
        }
    }
}

fn create_layer_popup(
    parent: &ZwlrLayerSurfaceV1,
    geometry: super::PopupPositioner,
    queue_handle: &QueueHandle<DispatchState>,
    compositor: &CompositorState,
    xdg_shell: &XdgShell,
    grab: Option<(&wl_seat::WlSeat, crate::InputSerial)>,
) -> Result<Popup, smithay_client_toolkit::error::GlobalError> {
    let positioner = XdgPositioner::new(xdg_shell)?;
    geometry.apply(&positioner);
    let surface = Surface::new(compositor, queue_handle)?;
    let popup = Popup::from_surface(None, &positioner, queue_handle, surface, xdg_shell)?;
    parent.get_popup(popup.xdg_popup());
    if let Some((seat, serial)) = grab {
        super::grab_popup(popup.xdg_popup(), seat, serial);
    }
    popup.wl_surface().commit();
    Ok(popup)
}

struct DispatchState {
    runtime: RcRuntime,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for DispatchState {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_compositor::WlCompositor, GlobalData> for DispatchState {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _: wl_compositor::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_region::WlRegion, ()> for DispatchState {
    fn event(
        _: &mut Self,
        _: &wl_region::WlRegion,
        _: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrLayerShellV1, ()> for DispatchState {
    fn event(
        _: &mut Self,
        _: &ZwlrLayerShellV1,
        _: <ZwlrLayerShellV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for DispatchState {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let mut runtime = state.runtime.borrow_mut();
        let id = runtime
            .windows
            .iter()
            .find_map(|(id, window)| match window {
                NativeWindow::Layer {
                    layer_surface: current,
                    ..
                } if current == layer_surface => Some(*id),
                _ => None,
            });
        let Some(id) = id else { return };
        match event {
            wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                layer_surface.ack_configure(serial);
                let size = PhysicalSize { width, height };
                if let Some(window) = runtime.windows.get(&id) {
                    window.handle().set_inner_size(size);
                    window.handle().set_ready();
                }
                runtime.events.push((id, WindowEvent::Resized(size)));
            }
            wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Event::Closed => {
                runtime.events.push((id, WindowEvent::CloseRequested));
            }
            _ => {}
        }
    }
}

impl Dispatch<WpCursorShapeManagerV1, GlobalData> for DispatchState {
    fn event(
        _: &mut Self,
        _: &WpCursorShapeManagerV1,
        _: <WpCursorShapeManagerV1 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpCursorShapeDeviceV1, GlobalData> for DispatchState {
    fn event(
        _: &mut Self,
        _: &WpCursorShapeDeviceV1,
        _: <WpCursorShapeDeviceV1 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for DispatchState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            let mut runtime = state.runtime.borrow_mut();
            if capabilities.contains(wl_seat::Capability::Pointer) && runtime.pointer.is_none() {
                let pointer = seat.get_pointer(qh, ());
                runtime.cursor_shape_device = runtime
                    .cursor_shape_manager
                    .as_ref()
                    .map(|manager| manager.get_shape_device(&pointer, qh));
                runtime.pointer = Some(pointer);
            }
            if capabilities.contains(wl_seat::Capability::Keyboard) && runtime.keyboard.is_none() {
                runtime.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for DispatchState {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let mut runtime = state.runtime.borrow_mut();
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                if let Some(id) = runtime.id_for_surface(&surface) {
                    let passthrough = runtime
                        .windows
                        .get(&id)
                        .is_some_and(|window| window.handle().pointer_passthrough());
                    runtime.pointer_over_passthrough = passthrough;
                    if passthrough {
                        if let Some(target) = runtime.last_pointer_target {
                            runtime.events.push((
                                target,
                                WindowEvent::CursorMoved {
                                    position: PhysicalPosition {
                                        x: surface_x,
                                        y: surface_y,
                                    },
                                },
                            ));
                        }
                    } else {
                        runtime.pointer_focus = Some(id);
                        runtime.last_pointer_target = Some(id);
                        runtime.cursor_serial = Some(serial);
                        runtime.events.push((
                            id,
                            WindowEvent::CursorMoved {
                                position: PhysicalPosition {
                                    x: surface_x,
                                    y: surface_y,
                                },
                            },
                        ));
                    }
                }
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                if let Some(id) = runtime.pointer_target() {
                    runtime.events.push((
                        id,
                        WindowEvent::CursorMoved {
                            position: PhysicalPosition {
                                x: surface_x,
                                y: surface_y,
                            },
                        },
                    ));
                }
            }
            wl_pointer::Event::Leave { surface, .. } => {
                if let Some(id) = runtime.id_for_surface(&surface) {
                    let passthrough = runtime
                        .windows
                        .get(&id)
                        .is_some_and(|window| window.handle().pointer_passthrough());
                    if passthrough {
                        runtime.pointer_over_passthrough = false;
                    } else if runtime.pointer_focus == Some(id) {
                        runtime.pointer_focus = None;
                        runtime.cursor_serial = None;
                        runtime.events.push((id, WindowEvent::CursorLeft));
                    }
                }
            }
            wl_pointer::Event::Button {
                serial,
                button,
                state: WEnum::Value(button_state),
                ..
            } => {
                if let Some(id) = runtime.pointer_target() {
                    runtime.events.push((
                        id,
                        WindowEvent::MouseInput {
                            pressed: button_state == wl_pointer::ButtonState::Pressed,
                            button: pointer_button(button),
                            serial: Some(crate::InputSerial(serial)),
                        },
                    ));
                }
            }
            wl_pointer::Event::Axis { axis, value, .. } => {
                let delta = match axis {
                    WEnum::Value(wl_pointer::Axis::VerticalScroll) => {
                        MouseScrollDelta::PixelDelta(PhysicalPosition { x: 0.0, y: value })
                    }
                    WEnum::Value(wl_pointer::Axis::HorizontalScroll) => {
                        MouseScrollDelta::PixelDelta(PhysicalPosition { x: value, y: 0.0 })
                    }
                    _ => return,
                };
                if let Some(id) = runtime.pointer_target() {
                    runtime.events.push((id, WindowEvent::MouseWheel { delta }));
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for DispatchState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let mut runtime = state.runtime.borrow_mut();
        match event {
            wl_keyboard::Event::Enter { surface, .. } => {
                if let Some(id) = runtime.id_for_surface(&surface) {
                    runtime.keyboard_focus = Some(id);
                    runtime.events.push((id, WindowEvent::Focused(true)));
                }
            }
            wl_keyboard::Event::Leave { surface, .. } => {
                if let Some(id) = runtime.id_for_surface(&surface) {
                    if runtime.keyboard_focus == Some(id) {
                        runtime.keyboard_focus = None;
                    }
                    runtime.events.push((id, WindowEvent::Focused(false)));
                }
            }
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(key_state),
                ..
            } => {
                if let Some(id) = runtime.keyboard_focus {
                    runtime.events.push((
                        id,
                        WindowEvent::KeyboardInput(KeyEvent {
                            key: keyboard_key(key),
                            pressed: key_state == wl_keyboard::KeyState::Pressed,
                            synthetic: false,
                        }),
                    ));
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_surface::WlSurface, SurfaceData> for DispatchState {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &SurfaceData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl WindowHandler for DispatchState {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &XdgWindow) {
        let mut runtime = self.runtime.borrow_mut();
        if let Some(id) = runtime.id_for_surface(window.wl_surface()) {
            runtime.events.push((id, WindowEvent::CloseRequested));
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        window: &XdgWindow,
        configure: WindowConfigure,
        _: u32,
    ) {
        let mut runtime = self.runtime.borrow_mut();
        let Some(id) = runtime.id_for_surface(window.wl_surface()) else {
            return;
        };
        let size = PhysicalSize {
            width: configure.new_size.0.map_or_else(
                || runtime.windows[&id].handle().inner_size().width,
                |size| size.get(),
            ),
            height: configure.new_size.1.map_or_else(
                || runtime.windows[&id].handle().inner_size().height,
                |size| size.get(),
            ),
        };
        if let Some(native) = runtime.windows.get(&id) {
            native.handle().set_inner_size(size);
            native.handle().set_ready();
        }
        runtime.events.push((id, WindowEvent::Resized(size)));
        runtime.events.push((
            id,
            WindowEvent::Focused(configure.state.contains(WindowState::ACTIVATED)),
        ));
    }
}

impl PopupHandler for DispatchState {
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        popup: &Popup,
        configure: PopupConfigure,
    ) {
        let mut runtime = self.runtime.borrow_mut();
        if let Some(id) = runtime.id_for_surface(popup.wl_surface()) {
            let size = PhysicalSize {
                width: configure.width.max(1) as u32,
                height: configure.height.max(1) as u32,
            };
            if let Some(native) = runtime.windows.get(&id) {
                native.handle().set_inner_size(size);
                native.handle().set_ready();
            }
            runtime.events.push((id, WindowEvent::Resized(size)));
            runtime.events.push((id, WindowEvent::RedrawRequested));
        }
    }

    fn done(&mut self, _: &Connection, _: &QueueHandle<Self>, popup: &Popup) {
        let mut runtime = self.runtime.borrow_mut();
        if let Some(id) = runtime.id_for_surface(popup.wl_surface()) {
            runtime.events.push((id, WindowEvent::Focused(false)));
            runtime.events.push((id, WindowEvent::PopupDone));
        }
    }
}

smithay_client_toolkit::delegate_xdg_shell!(DispatchState);
smithay_client_toolkit::delegate_xdg_window!(DispatchState);
smithay_client_toolkit::delegate_xdg_popup!(DispatchState);

pub struct Window {
    id: WindowId,
    surface: wl_surface::WlSurface,
    display: wl_display::WlDisplay,
    scale_factor: AtomicU32,
    inner_size: Mutex<PhysicalSize>,
    ready: AtomicBool,
    requested_redraw: AtomicBool,
    visible: AtomicBool,
    close_requests: Arc<Mutex<Vec<WindowId>>>,
    cursor_requests: Arc<Mutex<Vec<(WindowId, CursorIcon)>>>,
    pointer_passthrough: bool,
}

impl Window {
    fn new(
        id: WindowId,
        surface: wl_surface::WlSurface,
        display: wl_display::WlDisplay,
        close_requests: Arc<Mutex<Vec<WindowId>>>,
        cursor_requests: Arc<Mutex<Vec<(WindowId, CursorIcon)>>>,
        size: LogicalSize,
        pointer_passthrough: bool,
    ) -> Self {
        Self {
            id,
            surface,
            display,
            scale_factor: AtomicU32::new(1),
            inner_size: Mutex::new(PhysicalSize {
                width: size.width.ceil().max(1.0) as u32,
                height: size.height.ceil().max(1.0) as u32,
            }),
            ready: AtomicBool::new(false),
            requested_redraw: AtomicBool::new(true),
            visible: AtomicBool::new(false),
            close_requests,
            cursor_requests,
            pointer_passthrough,
        }
    }

    fn set_inner_size(&self, size: PhysicalSize) {
        *self.inner_size.lock().expect("window size lock poisoned") = size;
    }

    fn inner_size(&self) -> PhysicalSize {
        *self.inner_size.lock().expect("window size lock poisoned")
    }

    fn set_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }

    fn pointer_passthrough(&self) -> bool {
        self.pointer_passthrough
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let surface = proxy_pointer(&self.surface)?;
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::Wayland(WaylandWindowHandle::new(surface)))
        })
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let display = proxy_pointer(&self.display)?;
        Ok(unsafe {
            DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
                display,
            )))
        })
    }
}

impl PlatformWindow for Window {
    fn id(&self) -> WindowId {
        self.id
    }

    fn request_redraw(&self) {
        self.requested_redraw.store(true, Ordering::Release);
    }

    fn close(&self) {
        self.close_requests
            .lock()
            .expect("window close request lock poisoned")
            .push(self.id);
    }

    fn request_inner_size(&self, size: LogicalSize) {
        self.set_inner_size(PhysicalSize {
            width: size.width.ceil().max(1.0) as u32,
            height: size.height.ceil().max(1.0) as u32,
        });
        self.request_redraw();
    }

    fn set_outer_position(&self, _: LogicalPosition) {}
    fn outer_position(&self) -> Option<PhysicalPosition> {
        None
    }
    fn monitor_size(&self) -> Option<PhysicalSize> {
        None
    }
    fn scale_factor(&self) -> f64 {
        self.scale_factor.load(Ordering::Acquire) as f64
    }
    fn inner_size(&self) -> PhysicalSize {
        self.inner_size()
    }
    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
    fn set_visible(&self, visible: bool) {
        self.visible.store(visible, Ordering::Release);
    }
    fn set_minimized(&self, _: bool) {}
    fn set_maximized(&self, _: bool) {}
    fn set_window_level(&self, _: WindowLevel) {}
    fn drag_window(&self) -> Result<(), String> {
        Err("Wayland interactive move requires an input serial".to_owned())
    }
    fn drag_resize_window(&self, _: ResizeDirection) -> Result<(), String> {
        Err("Wayland interactive resize requires an input serial".to_owned())
    }
    fn set_cursor(&self, icon: CursorIcon) {
        let mut requests = self
            .cursor_requests
            .lock()
            .expect("cursor request lock poisoned");
        requests.retain(|(window_id, _)| *window_id != self.id);
        requests.push((self.id, icon));
    }
    fn focus(&self) {}
}

fn dispatch_redraws(runtime: &RcRuntime) {
    let mut runtime = runtime.borrow_mut();
    let redraws: Vec<_> = runtime
        .windows
        .iter()
        .filter_map(|(id, window)| {
            window
                .handle()
                .requested_redraw
                .swap(false, Ordering::AcqRel)
                .then_some(*id)
        })
        .collect();
    runtime.events.extend(
        redraws
            .into_iter()
            .map(|id| (id, WindowEvent::RedrawRequested)),
    );
}

fn timeout_for(state: &LoopState) -> Duration {
    match *state
        .control_flow
        .lock()
        .expect("control flow lock poisoned")
    {
        ControlFlow::Wait => USER_EVENT_POLL_INTERVAL,
        ControlFlow::WaitUntil(instant) => instant
            .saturating_duration_since(Instant::now())
            .min(USER_EVENT_POLL_INTERVAL),
    }
}

fn proxy_pointer<P: Proxy>(proxy: &P) -> Result<NonNull<c_void>, HandleError> {
    NonNull::new(proxy.id().as_ptr().cast()).ok_or(HandleError::Unavailable)
}

fn pointer_button(button: u32) -> MouseButton {
    match button {
        0x110 => MouseButton::Left,
        0x111 => MouseButton::Right,
        0x112 => MouseButton::Middle,
        value => MouseButton::Other(value.min(u16::MAX as u32) as u16),
    }
}

fn cursor_shape(icon: CursorIcon) -> Shape {
    match icon {
        CursorIcon::Default => Shape::Default,
        CursorIcon::Text => Shape::Text,
        CursorIcon::Pointer => Shape::Pointer,
        CursorIcon::NotAllowed => Shape::NotAllowed,
        CursorIcon::ResizeHorizontal => Shape::EwResize,
        CursorIcon::ResizeVertical => Shape::NsResize,
        CursorIcon::ResizeNwse => Shape::NwseResize,
        CursorIcon::ResizeNesw => Shape::NeswResize,
    }
}

fn keyboard_key(key: u32) -> Key {
    match key {
        1 => Key::Escape,
        14 => Key::Backspace,
        15 => Key::Tab,
        28 => Key::Enter,
        57 => Key::Space,
        102 => Key::Home,
        103 => Key::Up,
        105 => Key::Left,
        106 => Key::Right,
        107 => Key::End,
        108 => Key::Down,
        111 => Key::Delete,
        61 => Key::F3,
        _ => Key::Other,
    }
}

fn dispatch_with_timeout(
    connection: &Connection,
    queue: &mut smithay_client_toolkit::reexports::client::EventQueue<DispatchState>,
    timeout: Duration,
    state: &mut DispatchState,
) -> Result<(), String> {
    queue
        .dispatch_pending(state)
        .map_err(|error| error.to_string())?;
    connection.flush().map_err(|error| error.to_string())?;
    if let Some(guard) = connection.prepare_read() {
        let mut descriptor = libc::pollfd {
            fd: connection.backend().poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let milliseconds = timeout.as_millis().min(i32::MAX as u128) as i32;
        let ready = unsafe { libc::poll(&mut descriptor, 1, milliseconds) };
        if ready < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if ready > 0 {
            guard.read().map_err(|error| error.to_string())?;
        }
    }
    queue
        .dispatch_pending(state)
        .map_err(|error| error.to_string())?;
    Ok(())
}
