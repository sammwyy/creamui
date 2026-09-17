//! Common window-platform types for CreamUI.

mod types;
#[cfg(feature = "wayland")]
pub mod wayland;
#[cfg(not(all(feature = "wayland", target_os = "linux")))]
mod winit;

pub use self::types::*;
#[cfg(all(feature = "wayland", target_os = "linux"))]
pub use self::wayland::runtime::{
    ActiveEventLoop, ApplicationHandler, EventLoop, EventLoopBuilder, EventLoopProxy, Window,
};
#[cfg(not(all(feature = "wayland", target_os = "linux")))]
pub use self::winit::{
    ActiveEventLoop, ApplicationHandler, EventLoop, EventLoopBuilder, EventLoopProxy, Window,
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

pub trait PlatformWindow: HasDisplayHandle + HasWindowHandle + Send + Sync {
    fn id(&self) -> WindowId;
    fn request_redraw(&self);
    /// Called right before a frame is handed to the compositor, so the
    /// platform can throttle redraws to the display.
    fn pre_present_notify(&self) {}
    fn close(&self);
    fn request_inner_size(&self, size: LogicalSize);
    fn set_outer_position(&self, position: LogicalPosition);
    fn outer_position(&self) -> Option<PhysicalPosition>;
    fn monitor_size(&self) -> Option<PhysicalSize>;
    fn scale_factor(&self) -> f64;
    fn inner_size(&self) -> PhysicalSize;
    fn is_ready(&self) -> bool;
    fn set_visible(&self, visible: bool);
    fn set_minimized(&self, minimized: bool);
    fn set_maximized(&self, maximized: bool);
    fn set_window_level(&self, level: WindowLevel);
    fn drag_window(&self) -> Result<(), String>;
    fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), String>;
    fn set_cursor(&self, icon: CursorIcon);
    fn focus(&self);
    /// Starts a real drag-and-drop grab (e.g. a desktop icon dragged toward
    /// the dock or an external app), handed off to the compositor from
    /// `serial` — the input serial of the pointer-button-press that
    /// started it. `mime_types` are what the drag offers to a drop target.
    /// Default: unsupported, for backends with no such protocol.
    fn start_drag(
        &self,
        serial: InputSerial,
        mime_types: &[String],
        icon: Option<DragIcon>,
    ) -> Result<(), String> {
        let _ = (serial, mime_types, icon);
        Err("drag-and-drop is not supported on this backend".to_owned())
    }
    #[cfg(target_arch = "wasm32")]
    fn canvas(&self) -> Option<web_sys::HtmlCanvasElement>;
}

pub trait PlatformBackend {
    fn kind(&self) -> BackendKind;
    fn create_window(
        &self,
        attributes: WindowAttributes,
    ) -> Result<Arc<dyn PlatformWindow>, String>;
    fn create_popup(
        &self,
        attributes: WindowAttributes,
        popup: PopupOptions,
    ) -> Result<Arc<dyn PlatformWindow>, String>;
}
