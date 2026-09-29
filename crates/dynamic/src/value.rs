//! Value types shared verbatim with the loaded library via `creamui-abi` —
//! plain data, safe to construct and pass by value with no `unsafe` needed.

pub use creamui_abi::{
    CColor as Color, CDimension as Dimension, CStyle as Style, CTheme as Theme, ALIGN_BASELINE,
    ALIGN_CENTER, ALIGN_END, ALIGN_FLEX_END, ALIGN_FLEX_START, ALIGN_START, ALIGN_STRETCH,
    ALIGN_UNSET, FLEX_DIRECTION_COLUMN, FLEX_DIRECTION_COLUMN_REVERSE, FLEX_DIRECTION_ROW,
    FLEX_DIRECTION_ROW_REVERSE, JUSTIFY_CENTER, JUSTIFY_END, JUSTIFY_FLEX_END, JUSTIFY_FLEX_START,
    JUSTIFY_SPACE_AROUND, JUSTIFY_SPACE_BETWEEN, JUSTIFY_SPACE_EVENLY, JUSTIFY_START,
    JUSTIFY_STRETCH,
};

/// Which backend composites CreamUI's CPU-rasterized frame to the window —
/// see `creamui_render::RenderBackend` for the native equivalent. Can still
/// be force-overridden at launch with `CUI_OVERRIDE_RENDER_BACKEND=gpu|cpu`
/// regardless of what's set here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderBackend {
    #[default]
    Gpu,
    Cpu,
}

/// Compositor blur requested when a window is created.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlurRegion {
    Window,
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
}

/// Options for a window CreamUI opens, set once at creation. Mirrors
/// `creamui_render::WindowOptions`.
#[derive(Debug, Clone)]
pub struct WindowOptions {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub blur: Option<BlurRegion>,
    pub backend: RenderBackend,
}

impl Default for WindowOptions {
    fn default() -> Self {
        WindowOptions {
            title: "CreamUI".to_string(),
            width: 800,
            height: 600,
            resizable: true,
            decorations: true,
            transparent: false,
            blur: None,
            backend: RenderBackend::default(),
        }
    }
}

/// The logical-pixel size a `build_ui` closure should lay its root widget
/// out to fill. Mirrors `creamui_core::Size`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}
