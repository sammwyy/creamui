use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler as WinitApplicationHandler;
use winit::event::{
    ElementState, MouseButton as WinitMouseButton, MouseScrollDelta as WinitScrollDelta,
};
use winit::event_loop::{
    ActiveEventLoop as WinitActiveEventLoop, ControlFlow as WinitControlFlow,
    EventLoop as WinitEventLoop, EventLoopBuilder as WinitEventLoopBuilder,
    EventLoopProxy as WinitEventLoopProxy,
};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};
use winit::window::{
    CursorIcon as WinitCursorIcon, ResizeDirection as WinitResizeDirection,
    WindowAttributes as WinitWindowAttributes, WindowId as WinitWindowId,
    WindowLevel as WinitWindowLevel,
};

use crate::{
    BackendKind, ControlFlow, CursorIcon, Key, KeyEvent, LogicalPosition, LogicalSize, Modifiers,
    MouseButton, MouseScrollDelta, PhysicalPosition, PhysicalSize, PlatformBackend, PlatformWindow,
    PopupOptions, ResizeDirection, WindowAttributes, WindowEvent, WindowId, WindowLevel,
};

type WindowIds = Arc<Mutex<HashMap<WinitWindowId, WindowId>>>;

pub struct Window {
    inner: winit::window::Window,
    id: WindowId,
}

impl fmt::Debug for Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Window").field("id", &self.id()).finish()
    }
}

impl Window {
    pub fn id(&self) -> WindowId {
        self.id
    }
    pub fn request_redraw(&self) {
        self.inner.request_redraw();
    }
    pub fn request_inner_size(&self, size: LogicalSize) {
        let _ = self
            .inner
            .request_inner_size(winit::dpi::LogicalSize::new(size.width, size.height));
    }
    pub fn set_outer_position(&self, position: LogicalPosition) {
        self.inner
            .set_outer_position(winit::dpi::LogicalPosition::new(position.x, position.y));
    }
    pub fn outer_position(&self) -> Option<PhysicalPosition> {
        self.inner
            .outer_position()
            .ok()
            .map(|position| PhysicalPosition {
                x: position.x as f64,
                y: position.y as f64,
            })
    }
    pub fn monitor_size(&self) -> Option<PhysicalSize> {
        self.inner.current_monitor().map(|monitor| {
            let size = monitor.size();
            PhysicalSize {
                width: size.width,
                height: size.height,
            }
        })
    }
    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }
    pub fn inner_size(&self) -> PhysicalSize {
        let size = self.inner.inner_size();
        PhysicalSize {
            width: size.width,
            height: size.height,
        }
    }
    pub fn set_window_level(&self, level: WindowLevel) {
        self.inner.set_window_level(match level {
            WindowLevel::Normal => WinitWindowLevel::Normal,
            WindowLevel::AlwaysOnTop => WinitWindowLevel::AlwaysOnTop,
        });
    }
    pub fn set_visible(&self, visible: bool) {
        self.inner.set_visible(visible);
    }
    pub fn set_minimized(&self, minimized: bool) {
        self.inner.set_minimized(minimized);
    }
    pub fn set_maximized(&self, maximized: bool) {
        self.inner.set_maximized(maximized);
    }
    pub fn focus(&self) {
        self.inner.focus_window();
    }
    pub fn drag_window(&self) -> Result<(), String> {
        self.inner.drag_window().map_err(|error| error.to_string())
    }
    pub fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), String> {
        self.inner
            .drag_resize_window(to_winit_resize_direction(direction))
            .map_err(|error| error.to_string())
    }
    pub fn set_cursor(&self, icon: CursorIcon) {
        self.inner.set_cursor(to_winit_cursor(icon));
    }
    #[cfg(target_arch = "wasm32")]
    pub fn canvas(&self) -> Option<web_sys::HtmlCanvasElement> {
        use winit::platform::web::WindowExtWebSys;
        self.inner.canvas()
    }
}

impl PlatformWindow for Window {
    fn id(&self) -> WindowId {
        self.id()
    }
    fn request_redraw(&self) {
        self.request_redraw();
    }
    fn close(&self) {}
    fn request_inner_size(&self, size: LogicalSize) {
        self.request_inner_size(size);
    }
    fn set_outer_position(&self, position: LogicalPosition) {
        self.set_outer_position(position);
    }
    fn outer_position(&self) -> Option<PhysicalPosition> {
        self.outer_position()
    }
    fn monitor_size(&self) -> Option<PhysicalSize> {
        self.monitor_size()
    }
    fn scale_factor(&self) -> f64 {
        self.scale_factor()
    }
    fn inner_size(&self) -> PhysicalSize {
        self.inner_size()
    }
    fn is_ready(&self) -> bool {
        true
    }
    fn set_visible(&self, visible: bool) {
        self.set_visible(visible);
    }
    fn set_minimized(&self, minimized: bool) {
        self.set_minimized(minimized);
    }
    fn set_maximized(&self, maximized: bool) {
        self.set_maximized(maximized);
    }
    fn set_window_level(&self, level: WindowLevel) {
        self.set_window_level(level);
    }
    fn drag_window(&self) -> Result<(), String> {
        self.drag_window()
    }
    fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), String> {
        self.drag_resize_window(direction)
    }
    fn set_cursor(&self, icon: CursorIcon) {
        self.set_cursor(icon);
    }
    fn focus(&self) {
        self.focus();
    }
    #[cfg(target_arch = "wasm32")]
    fn canvas(&self) -> Option<web_sys::HtmlCanvasElement> {
        self.canvas()
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        self.inner.window_handle()
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        self.inner.display_handle()
    }
}

pub struct ActiveEventLoop<'a> {
    inner: &'a WinitActiveEventLoop,
    ids: &'a WindowIds,
}

impl ActiveEventLoop<'_> {
    pub fn create_window(
        &self,
        attributes: WindowAttributes,
    ) -> Result<Arc<dyn PlatformWindow>, String> {
        let mut inner = WinitWindowAttributes::default()
            .with_title(attributes.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                attributes.size.width,
                attributes.size.height,
            ))
            .with_resizable(attributes.resizable)
            .with_decorations(attributes.decorations)
            .with_transparent(attributes.transparent);
        if let Some(position) = attributes.position {
            inner = inner.with_position(winit::dpi::LogicalPosition::new(position.x, position.y));
        }
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            inner = inner.with_append(true);
        }
        self.inner
            .create_window(inner)
            .map(|inner| {
                let id = WindowId::next();
                self.ids
                    .lock()
                    .expect("window IDs lock poisoned")
                    .insert(inner.id(), id);
                Arc::new(Window { inner, id }) as Arc<dyn PlatformWindow>
            })
            .map_err(|error| error.to_string())
    }
    pub fn create_popup(
        &self,
        attributes: WindowAttributes,
        popup: PopupOptions,
    ) -> Result<Arc<dyn PlatformWindow>, String> {
        let _ = popup;
        self.create_window(attributes)
    }
    pub fn exit(&self) {
        self.inner.exit();
    }
    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        self.inner.set_control_flow(match control_flow {
            ControlFlow::Wait => WinitControlFlow::Wait,
            ControlFlow::WaitUntil(instant) => WinitControlFlow::WaitUntil(instant),
        });
    }
}

impl PlatformBackend for ActiveEventLoop<'_> {
    fn kind(&self) -> BackendKind {
        BackendKind::Winit
    }

    fn create_window(
        &self,
        attributes: WindowAttributes,
    ) -> Result<Arc<dyn PlatformWindow>, String> {
        ActiveEventLoop::create_window(self, attributes)
    }

    fn create_popup(
        &self,
        attributes: WindowAttributes,
        popup: PopupOptions,
    ) -> Result<Arc<dyn PlatformWindow>, String> {
        ActiveEventLoop::create_popup(self, attributes, popup)
    }
}

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

pub struct EventLoop<T: 'static> {
    inner: WinitEventLoop<T>,
    ids: WindowIds,
}
pub struct EventLoopBuilder<T: 'static> {
    inner: WinitEventLoopBuilder<T>,
}
pub struct EventLoopProxy<T: 'static> {
    inner: WinitEventLoopProxy<T>,
}

impl<T: 'static> EventLoop<T> {
    pub fn with_user_event() -> EventLoopBuilder<T> {
        EventLoopBuilder {
            inner: WinitEventLoop::with_user_event(),
        }
    }
    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            inner: self.inner.create_proxy(),
        }
    }
    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        self.inner.set_control_flow(match control_flow {
            ControlFlow::Wait => WinitControlFlow::Wait,
            ControlFlow::WaitUntil(instant) => WinitControlFlow::WaitUntil(instant),
        });
    }
    pub fn run_app<H: ApplicationHandler<T>>(self, handler: &mut H) -> Result<(), String> {
        self.inner
            .run_app(&mut Adapter {
                handler,
                ids: self.ids,
            })
            .map_err(|error| error.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    pub fn spawn_app<H: ApplicationHandler<T> + 'static>(self, handler: H) {
        use winit::platform::web::EventLoopExtWebSys;
        self.inner.spawn_app(Adapter {
            handler,
            ids: self.ids,
        });
    }
}

impl<T: 'static> EventLoopBuilder<T> {
    #[cfg(target_os = "linux")]
    pub fn with_any_thread(&mut self, any_thread: bool) {
        use winit::platform::x11::EventLoopBuilderExtX11;
        self.inner.with_any_thread(any_thread);
    }

    pub fn build(mut self) -> Result<EventLoop<T>, String> {
        self.inner
            .build()
            .map(|inner| EventLoop {
                inner,
                ids: Arc::new(Mutex::new(HashMap::new())),
            })
            .map_err(|error| error.to_string())
    }
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), String> {
        self.inner
            .send_event(event)
            .map_err(|error| error.to_string())
    }
}

struct Adapter<H> {
    handler: H,
    ids: WindowIds,
}

impl<T: 'static, H: ApplicationHandler<T>> WinitApplicationHandler<T> for Adapter<&mut H> {
    fn resumed(&mut self, event_loop: &WinitActiveEventLoop) {
        self.handler.resumed(&ActiveEventLoop {
            inner: event_loop,
            ids: &self.ids,
        });
    }
    fn window_event(
        &mut self,
        event_loop: &WinitActiveEventLoop,
        window_id: WinitWindowId,
        event: winit::event::WindowEvent,
    ) {
        self.handler.window_event(
            &ActiveEventLoop {
                inner: event_loop,
                ids: &self.ids,
            },
            self.ids.lock().expect("window IDs lock poisoned")[&window_id],
            from_winit_window_event(event),
        );
    }
    fn user_event(&mut self, event_loop: &WinitActiveEventLoop, event: T) {
        self.handler.user_event(
            &ActiveEventLoop {
                inner: event_loop,
                ids: &self.ids,
            },
            event,
        );
    }
    fn about_to_wait(&mut self, event_loop: &WinitActiveEventLoop) {
        self.handler.about_to_wait(&ActiveEventLoop {
            inner: event_loop,
            ids: &self.ids,
        });
    }
}

#[cfg(target_arch = "wasm32")]
impl<T: 'static, H: ApplicationHandler<T>> WinitApplicationHandler<T> for Adapter<H> {
    fn resumed(&mut self, event_loop: &WinitActiveEventLoop) {
        self.handler.resumed(&ActiveEventLoop {
            inner: event_loop,
            ids: &self.ids,
        });
    }
    fn window_event(
        &mut self,
        event_loop: &WinitActiveEventLoop,
        window_id: WinitWindowId,
        event: winit::event::WindowEvent,
    ) {
        self.handler.window_event(
            &ActiveEventLoop {
                inner: event_loop,
                ids: &self.ids,
            },
            self.ids.lock().expect("window IDs lock poisoned")[&window_id],
            from_winit_window_event(event),
        );
    }
    fn user_event(&mut self, event_loop: &WinitActiveEventLoop, event: T) {
        self.handler.user_event(
            &ActiveEventLoop {
                inner: event_loop,
                ids: &self.ids,
            },
            event,
        );
    }
    fn about_to_wait(&mut self, event_loop: &WinitActiveEventLoop) {
        self.handler.about_to_wait(&ActiveEventLoop {
            inner: event_loop,
            ids: &self.ids,
        });
    }
}

fn from_winit_window_event(event: winit::event::WindowEvent) -> WindowEvent {
    match event {
        winit::event::WindowEvent::CloseRequested => WindowEvent::CloseRequested,
        winit::event::WindowEvent::Resized(size) => WindowEvent::Resized(PhysicalSize {
            width: size.width,
            height: size.height,
        }),
        winit::event::WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            WindowEvent::ScaleFactorChanged { scale_factor }
        }
        winit::event::WindowEvent::CursorMoved { position, .. } => WindowEvent::CursorMoved {
            position: PhysicalPosition {
                x: position.x,
                y: position.y,
            },
        },
        winit::event::WindowEvent::MouseInput { state, button, .. } => WindowEvent::MouseInput {
            pressed: state == ElementState::Pressed,
            button: from_winit_mouse_button(button),
            serial: None,
        },
        winit::event::WindowEvent::CursorLeft { .. } => WindowEvent::CursorLeft,
        winit::event::WindowEvent::Focused(focused) => WindowEvent::Focused(focused),
        winit::event::WindowEvent::MouseWheel { delta, .. } => WindowEvent::MouseWheel {
            delta: match delta {
                WinitScrollDelta::LineDelta(x, y) => MouseScrollDelta::LineDelta(x, y),
                WinitScrollDelta::PixelDelta(position) => {
                    MouseScrollDelta::PixelDelta(PhysicalPosition {
                        x: position.x,
                        y: position.y,
                    })
                }
            },
        },
        winit::event::WindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } => WindowEvent::KeyboardInput(KeyEvent {
            key: from_winit_key(&event.logical_key),
            pressed: event.state == ElementState::Pressed,
            synthetic: is_synthetic,
        }),
        winit::event::WindowEvent::ModifiersChanged(modifiers) => {
            WindowEvent::ModifiersChanged(from_winit_modifiers(modifiers.state()))
        }
        winit::event::WindowEvent::RedrawRequested => WindowEvent::RedrawRequested,
        _ => WindowEvent::Other,
    }
}

fn from_winit_key(key: &WinitKey) -> Key {
    match key {
        WinitKey::Character(value) => Key::Character(value.to_string()),
        WinitKey::Named(NamedKey::Space) => Key::Space,
        WinitKey::Named(NamedKey::Backspace) => Key::Backspace,
        WinitKey::Named(NamedKey::Delete) => Key::Delete,
        WinitKey::Named(NamedKey::Enter) => Key::Enter,
        WinitKey::Named(NamedKey::Tab) => Key::Tab,
        WinitKey::Named(NamedKey::Escape) => Key::Escape,
        WinitKey::Named(NamedKey::ArrowLeft) => Key::Left,
        WinitKey::Named(NamedKey::ArrowRight) => Key::Right,
        WinitKey::Named(NamedKey::ArrowUp) => Key::Up,
        WinitKey::Named(NamedKey::ArrowDown) => Key::Down,
        WinitKey::Named(NamedKey::Home) => Key::Home,
        WinitKey::Named(NamedKey::End) => Key::End,
        WinitKey::Named(NamedKey::F3) => Key::F3,
        _ => Key::Other,
    }
}

fn from_winit_modifiers(modifiers: ModifiersState) -> Modifiers {
    Modifiers {
        ctrl: modifiers.control_key(),
        shift: modifiers.shift_key(),
    }
}
fn from_winit_mouse_button(button: WinitMouseButton) -> MouseButton {
    match button {
        WinitMouseButton::Left => MouseButton::Left,
        WinitMouseButton::Right => MouseButton::Right,
        WinitMouseButton::Middle => MouseButton::Middle,
        WinitMouseButton::Back => MouseButton::Other(4),
        WinitMouseButton::Forward => MouseButton::Other(5),
        WinitMouseButton::Other(value) => MouseButton::Other(value),
    }
}
fn to_winit_cursor(icon: CursorIcon) -> WinitCursorIcon {
    match icon {
        CursorIcon::Default => WinitCursorIcon::Default,
        CursorIcon::Text => WinitCursorIcon::Text,
        CursorIcon::Pointer => WinitCursorIcon::Pointer,
        CursorIcon::NotAllowed => WinitCursorIcon::NotAllowed,
        CursorIcon::ResizeHorizontal => WinitCursorIcon::EwResize,
        CursorIcon::ResizeVertical => WinitCursorIcon::NsResize,
        CursorIcon::ResizeNwse => WinitCursorIcon::NwseResize,
        CursorIcon::ResizeNesw => WinitCursorIcon::NeswResize,
    }
}
fn to_winit_resize_direction(direction: ResizeDirection) -> WinitResizeDirection {
    match direction {
        ResizeDirection::East => WinitResizeDirection::East,
        ResizeDirection::West => WinitResizeDirection::West,
        ResizeDirection::North => WinitResizeDirection::North,
        ResizeDirection::South => WinitResizeDirection::South,
        ResizeDirection::NorthWest => WinitResizeDirection::NorthWest,
        ResizeDirection::NorthEast => WinitResizeDirection::NorthEast,
        ResizeDirection::SouthWest => WinitResizeDirection::SouthWest,
        ResizeDirection::SouthEast => WinitResizeDirection::SouthEast,
    }
}
