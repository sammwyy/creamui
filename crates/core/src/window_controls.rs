//! Window state and actions for titlebars drawn inside the window.

use crate::Rect;
use std::rc::Rc;

/// Who draws the minimize, maximize and close controls of an in-window titlebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TitlebarMode {
    /// The platform has not answered yet. Reserve nothing and draw no fallback.
    #[default]
    Pending,
    /// Platform-drawn controls overlay the content inside [`TitlebarState::overlay`].
    Overlay,
    /// The application draws its own controls.
    Client,
    /// A platform frame sits outside the content, so the titlebar draws no controls.
    Server,
    /// The window is undecorated by request.
    None,
}

/// One of the standard window buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowButton {
    Minimize,
    Maximize,
    Close,
}

/// Decoration result observed by an in-window titlebar.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TitlebarState {
    pub mode: TitlebarMode,
    /// Logical rectangle covered by platform controls in [`TitlebarMode::Overlay`].
    pub overlay: Rect,
    pub maximized: bool,
    /// A button whose input the platform handles itself (for example the
    /// maximize button on Windows, which opens snap layouts) while the pointer
    /// hovers it. The titlebar paints that button's hover state from here.
    pub hovered: Option<WindowButton>,
}

/// Platform side of [`WindowControls`], implemented by the renderer.
pub trait WindowControlsBackend {
    /// Reading the state while building widgets subscribes to its changes.
    fn state(&self) -> TitlebarState;
    /// Asks the platform for the best titlebar integration it supports:
    /// overlay controls when available, client-side controls otherwise.
    /// Repeated calls are no-ops.
    fn request_titlebar(&self);
    fn minimize(&self);
    fn toggle_maximize(&self);
    fn close(&self);
}

/// Window state and actions a window provides to the widgets it builds.
#[derive(Clone)]
pub struct WindowControls(Rc<dyn WindowControlsBackend>);

impl WindowControls {
    pub fn new(backend: impl WindowControlsBackend + 'static) -> Self {
        Self(Rc::new(backend))
    }

    pub fn state(&self) -> TitlebarState {
        self.0.state()
    }

    pub fn request_titlebar(&self) {
        self.0.request_titlebar();
    }

    pub fn minimize(&self) {
        self.0.minimize();
    }

    pub fn toggle_maximize(&self) {
        self.0.toggle_maximize();
    }

    pub fn close(&self) {
        self.0.close();
    }

    pub fn press(&self, button: WindowButton) {
        match button {
            WindowButton::Minimize => self.minimize(),
            WindowButton::Maximize => self.toggle_maximize(),
            WindowButton::Close => self.close(),
        }
    }
}
