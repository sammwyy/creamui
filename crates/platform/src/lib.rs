//! Common window-platform types for CreamUI.

mod winit;

pub use self::winit::{
    ActiveEventLoop, ApplicationHandler, ControlFlow, CursorIcon, EventLoop, EventLoopBuilder,
    EventLoopProxy, InputSerial, Key, KeyEvent, LogicalPosition, LogicalSize, Modifiers,
    MouseButton, MouseScrollDelta, PhysicalPosition, PhysicalSize, PopupOptions, ResizeDirection,
    Window, WindowAttributes, WindowEvent, WindowId, WindowLevel,
};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Winit,
    Wayland,
    X11,
    Windows,
}

pub trait PlatformWindow: HasDisplayHandle + HasWindowHandle {
    fn id(&self) -> WindowId;
    fn request_redraw(&self);
    fn request_inner_size(&self, size: LogicalSize);
    fn set_outer_position(&self, position: LogicalPosition);
    fn outer_position(&self) -> Option<PhysicalPosition>;
    fn monitor_size(&self) -> Option<PhysicalSize>;
    fn scale_factor(&self) -> f64;
    fn inner_size(&self) -> PhysicalSize;
    fn set_visible(&self, visible: bool);
    fn set_minimized(&self, minimized: bool);
    fn focus(&self);
}

pub trait PlatformBackend {
    fn kind(&self) -> BackendKind;
    fn create_window(&self, attributes: WindowAttributes) -> Result<Arc<Window>, String>;
    fn create_popup(
        &self,
        attributes: WindowAttributes,
        popup: PopupOptions,
    ) -> Result<Arc<Window>, String>;
}
