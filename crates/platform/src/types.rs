use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
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
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    F3,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupPlacement {
    Above,
    Below,
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
    MouseWheel {
        delta: MouseScrollDelta,
    },
    KeyboardInput(KeyEvent),
    ModifiersChanged(Modifiers),
    PopupDone,
    RedrawRequested,
    Other,
}

pub enum ControlFlow {
    Wait,
    WaitUntil(Instant),
}
