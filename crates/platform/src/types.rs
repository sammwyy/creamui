use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub(crate) u64);

impl WindowId {
    pub(crate) fn next() -> Self {
        static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

impl LogicalSize {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalPosition {
    pub x: f64,
    pub y: f64,
}

impl LogicalPosition {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalPosition {
    pub x: f64,
    pub y: f64,
}

/// Logical-pixel bands of the window covered by system UI.
///
/// The status bar, navigation bar, display cutout, and on-screen keyboard
/// sit in these bands. Taps there are delivered to the system, so
/// interactive content has to be laid out inside the rectangle they leave
/// free. [`SafeArea::ZERO`] means the window's client area is already clear
/// of that chrome, which is the case for ordinary desktop windows.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SafeArea {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl SafeArea {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    pub fn is_empty(self) -> bool {
        self == Self::ZERO
    }

    /// Component-wise maximum. Used when two platform reports describe the
    /// same occupied bands in different ways.
    pub fn max(self, other: Self) -> Self {
        Self {
            top: self.top.max(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
            left: self.left.max(other.left),
        }
    }

    /// Shrinks each edge so the bands still fit inside a viewport of
    /// `width` by `height`. A band that would consume the whole axis is
    /// kept on the start edge and the opposite edge becomes zero.
    pub fn clamp(self, width: f32, height: f32) -> Self {
        let top = self.top.max(0.0).min(height.max(0.0));
        let bottom = self.bottom.max(0.0).min((height - top).max(0.0));
        let left = self.left.max(0.0).min(width.max(0.0));
        let right = self.right.max(0.0).min((width - left).max(0.0));
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Converts physical-pixel insets into logical pixels. Negative edges
    /// are treated as empty.
    pub fn from_physical_px(top: i32, right: i32, bottom: i32, left: i32, scale: f64) -> Self {
        let scale = if scale.is_finite() && scale > f64::EPSILON {
            scale
        } else {
            1.0
        };
        let logical = |px: i32| ((px.max(0) as f64) / scale) as f32;
        Self {
            top: logical(top),
            right: logical(right),
            bottom: logical(bottom),
            left: logical(left),
        }
    }

    /// Insets of an exclusive content rectangle inside a surface.
    ///
    /// `left`/`top`/`right`/`bottom` are edges in the same pixel space as
    /// the surface, matching the native content rectangle. An empty rect,
    /// or one that does not overlap the surface, produces [`SafeArea::ZERO`]
    /// so a report that has not arrived yet does not pad the whole window.
    pub fn from_content_rect(
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        surface_width: u32,
        surface_height: u32,
        scale: f64,
    ) -> Self {
        if right <= left || bottom <= top {
            return Self::ZERO;
        }
        let surface_width = surface_width as i32;
        let surface_height = surface_height as i32;
        if surface_width <= 0
            || surface_height <= 0
            || left >= surface_width
            || top >= surface_height
            || right <= 0
            || bottom <= 0
        {
            return Self::ZERO;
        }
        Self::from_physical_px(
            top,
            surface_width - right,
            surface_height - bottom,
            left,
            scale,
        )
    }
}

/// Region behind a window's surface the compositor should blur, via
/// [`crate::PlatformWindow::set_blur_region`]. Coordinates are logical
/// window-local pixels.
#[derive(Debug, Clone, PartialEq)]
pub enum BlurRegion {
    /// Follow painted translucent widget backgrounds, including their corners.
    Content,
    /// Exact window-local coverage as integer rectangles (a Wayland region).
    Regions(std::sync::Arc<[(i32, i32, i32, i32)]>),
    /// Blur behind the whole surface.
    Window,
    /// Blur restricted to this rectangle.
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
}

/// Optional compositor integration requested by a client window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorIntegrationRequest {
    Hybrid,
}

/// Effective integration selected by the compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorIntegrationMode {
    None,
    Hybrid,
}

/// Client-local rectangle occupied by compositor controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompositorControls {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Who owns the effective window frame, not merely what the client requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowDecorationMode {
    #[default]
    None,
    /// Waiting for the compositor's first configure; do not draw a fallback yet.
    Pending,
    /// The compositor (or native window system) owns the titlebar and controls.
    Server,
    /// The compositor requires the application to supply its own controls.
    Client,
    /// Server-owned controls overlay the client surface, without an external titlebar.
    Hybrid,
}

/// Negotiated frame and the logical-pixel rectangle reserved for overlay controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowDecorations {
    pub mode: WindowDecorationMode,
    pub controls: CompositorControls,
}

/// Standard xdg-decoration and optional overlay integration negotiate independently.
/// Keep both results so withdrawing the overlay restores the standard frame.
#[cfg(all(feature = "wayland", target_os = "linux"))]
pub(crate) struct DecorationNegotiation {
    requested: bool,
    standard: WindowDecorationMode,
    hybrid: Option<CompositorControls>,
}

#[cfg(all(feature = "wayland", target_os = "linux"))]
impl DecorationNegotiation {
    pub(crate) fn new(requested: bool) -> Self {
        Self {
            requested,
            standard: if requested {
                WindowDecorationMode::Pending
            } else {
                WindowDecorationMode::None
            },
            hybrid: None,
        }
    }

    pub(crate) fn configure(&mut self, server: bool) -> WindowDecorations {
        self.standard = if server {
            WindowDecorationMode::Server
        } else if self.requested {
            WindowDecorationMode::Client
        } else {
            WindowDecorationMode::None
        };
        self.effective()
    }

    pub(crate) fn integrate(
        &mut self,
        mode: CompositorIntegrationMode,
        controls: CompositorControls,
    ) -> WindowDecorations {
        self.hybrid = (mode == CompositorIntegrationMode::Hybrid).then_some(controls);
        self.effective()
    }

    pub(crate) fn effective(&self) -> WindowDecorations {
        match self.hybrid {
            Some(controls) => WindowDecorations {
                mode: WindowDecorationMode::Hybrid,
                controls,
            },
            None => WindowDecorations {
                mode: self.standard,
                controls: CompositorControls::default(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowLevel {
    Normal,
    AlwaysOnTop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowRole {
    #[default]
    Normal,
    Desktop,
    Overlay,
    TopPanel,
    BottomPanel,
    LeftPanel,
    RightPanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorIcon {
    Default,
    Text,
    Pointer,
    NotAllowed,
    ResizeHorizontal,
    ResizeVertical,
    ResizeNwse,
    ResizeNesw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    East,
    West,
    North,
    South,
    NorthWest,
    NorthEast,
    SouthWest,
    SouthEast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseScrollDelta {
    LineDelta(f32, f32),
    PixelDelta(PhysicalPosition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub logo: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Character(String),
    Space,
    Backspace,
    Delete,
    Enter,
    Tab,
    Escape,
    Back,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub pressed: bool,
    pub synthetic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputSerial(pub u32);

/// A bitmap shown next to the cursor for [`crate::PlatformWindow::start_drag`].
#[derive(Debug, Clone)]
pub struct DragIcon {
    /// Straight (non-premultiplied) RGBA8, `width * height * 4` bytes.
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupPlacement {
    Above,
    Below,
    RightTop,
    RightCenter,
    RightBottom,
    LeftTop,
    LeftCenter,
    LeftBottom,
}

#[derive(Debug, Clone)]
pub struct PopupOptions {
    pub parent: WindowId,
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub anchor_width: f32,
    pub anchor_height: f32,
    pub input_serial: Option<InputSerial>,
    pub placement: PopupPlacement,
}

#[derive(Debug, Clone)]
pub struct WindowAttributes {
    pub title: String,
    pub size: LogicalSize,
    pub position: Option<LogicalPosition>,
    pub resizable: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub role: WindowRole,
}

impl Default for WindowAttributes {
    fn default() -> Self {
        Self {
            title: String::new(),
            size: LogicalSize::new(800.0, 600.0),
            position: None,
            resizable: true,
            decorations: true,
            transparent: false,
            role: WindowRole::Normal,
        }
    }
}

#[derive(Debug, Clone)]
pub enum WindowEvent {
    /// Final decoration ownership changed, including custom compositor overlays.
    DecorationsChanged(WindowDecorations),
    CloseRequested,
    Resized(PhysicalSize),
    ScaleFactorChanged {
        scale_factor: f64,
    },
    CursorMoved {
        position: PhysicalPosition,
    },
    MouseInput {
        pressed: bool,
        button: MouseButton,
        serial: Option<InputSerial>,
    },
    CursorLeft,
    Focused(bool),
    /// The compositor accepted, changed, or removed a requested integration.
    /// Content can reserve [`CompositorControls`] to avoid overlapping it.
    CompositorIntegration {
        mode: CompositorIntegrationMode,
        controls: CompositorControls,
    },
    MouseWheel {
        delta: MouseScrollDelta,
    },
    Touch {
        id: u64,
        phase: TouchPhase,
        position: PhysicalPosition,
    },
    KeyboardInput(KeyEvent),
    ModifiersChanged(Modifiers),
    PopupDone,
    RedrawRequested,
    /// Drag-and-drop tracking (replaces normal pointer motion during a
    /// [`crate::PlatformWindow::start_drag`] grab).
    DragEntered {
        position: PhysicalPosition,
    },
    DragMoved {
        position: PhysicalPosition,
    },
    DragLeft,
    DragDropped,
    Other,
}

pub enum ControlFlow {
    Wait,
    WaitUntil(Instant),
}

#[cfg(test)]
mod tests {
    use super::SafeArea;

    #[cfg(all(feature = "wayland", target_os = "linux"))]
    #[test]
    fn decoration_negotiation_tracks_the_compositor_not_the_request() {
        use super::*;
        let mut state = DecorationNegotiation::new(true);
        assert_eq!(state.effective().mode, WindowDecorationMode::Pending);
        assert_eq!(state.configure(false).mode, WindowDecorationMode::Client);
        assert_eq!(state.configure(true).mode, WindowDecorationMode::Server);
        let controls = CompositorControls {
            x: 992,
            y: 14,
            width: 112,
            height: 28,
        };
        assert_eq!(
            state.integrate(CompositorIntegrationMode::Hybrid, controls),
            WindowDecorations {
                mode: WindowDecorationMode::Hybrid,
                controls
            }
        );
        // A standard configure must not erase a negotiated overlay.
        assert_eq!(state.configure(false).mode, WindowDecorationMode::Hybrid);
        assert_eq!(
            state.integrate(CompositorIntegrationMode::None, controls),
            WindowDecorations {
                mode: WindowDecorationMode::Client,
                controls: CompositorControls::default()
            }
        );
        state.configure(true);
        state.integrate(CompositorIntegrationMode::Hybrid, controls);
        assert_eq!(
            state
                .integrate(CompositorIntegrationMode::None, controls)
                .mode,
            WindowDecorationMode::Server
        );

        let mut undecorated = DecorationNegotiation::new(false);
        assert_eq!(
            undecorated.configure(false).mode,
            WindowDecorationMode::None
        );
        // A compositor may force server decoration even if none was requested.
        assert_eq!(
            undecorated.configure(true).mode,
            WindowDecorationMode::Server
        );
    }

    #[test]
    fn content_rect_insets_are_the_bands_outside_the_rect() {
        let area = SafeArea::from_content_rect(0, 80, 1080, 2200, 1080, 2400, 2.0);
        assert_eq!(
            area,
            SafeArea {
                top: 40.0,
                right: 0.0,
                bottom: 100.0,
                left: 0.0,
            }
        );
    }

    #[test]
    fn an_empty_or_full_content_rect_has_no_insets() {
        assert!(SafeArea::from_content_rect(0, 0, 0, 0, 1080, 2400, 2.0).is_empty());
        assert!(SafeArea::from_content_rect(0, 0, 1080, 2400, 1080, 2400, 2.0).is_empty());
        assert!(SafeArea::from_content_rect(0, 3000, 10, 3010, 1080, 2400, 1.0).is_empty());
    }

    #[test]
    fn physical_insets_convert_to_logical_pixels() {
        assert_eq!(
            SafeArea::from_physical_px(48, 0, 24, 12, 2.0),
            SafeArea {
                top: 24.0,
                right: 0.0,
                bottom: 12.0,
                left: 6.0,
            }
        );
        assert!(SafeArea::from_physical_px(-4, 0, 0, 0, 0.0).is_empty());
    }

    #[test]
    fn clamp_keeps_insets_inside_the_viewport() {
        let area = SafeArea {
            top: 100.0,
            right: 80.0,
            bottom: 40.0,
            left: 30.0,
        };
        assert_eq!(
            area.clamp(90.0, 50.0),
            SafeArea {
                top: 50.0,
                right: 60.0,
                bottom: 0.0,
                left: 30.0,
            }
        );
    }
}
