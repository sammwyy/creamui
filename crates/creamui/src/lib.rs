//! The main entry point for CreamUI applications.
//!
//! The default feature set includes the native runtime: widgets, theming,
//! reactivity, layout, and window rendering. Enable optional integrations as
//! needed, such as `image`, `jsx`, `dynamic`, `ffi`, or `devtools`.

pub use creamui_core as core;
pub use creamui_fonts as fonts;
pub use creamui_reactive as reactive;
pub use creamui_render as render;
pub use creamui_theme as theme;
pub use creamui_widgets as widgets;

pub use creamui_core::{
    Border, BoxedWidget, ColorToken, ColorValue, InteractionState, LengthValue, PaintStyle,
    Painter, Size, StateStyle, Style, StyleParseError, StyleProp, StyleState, Styled,
    TypographyStyle, Widget,
};
#[cfg(feature = "devtools")]
pub use creamui_devtools as devtools;
pub use creamui_fonts::{include_font, use_font, FontHandle, FontWeight};
pub use creamui_reactive::{create_effect, Effect, Signal};
pub use creamui_render::{
    run, AppBuilder, AppHandle, CloseBehavior, PanicDetails, RenderBackend, WindowHandle,
    WindowOptions,
};
#[cfg(all(feature = "tray", target_os = "linux"))]
pub use creamui_render::{TrayBuilder, TrayIcon};
pub use creamui_theme::{use_theme, Color, ColorScheme, Theme, ThemeProvider, Typography};
pub use creamui_widgets::*;

#[cfg(feature = "image")]
pub use creamui_image as image;
#[cfg(feature = "image")]
pub use creamui_image::{Image, ImageData, ImageError, ImageFit};

#[cfg(feature = "jsx")]
pub use creamui_jsx as jsx_runtime;
#[cfg(feature = "jsx")]
pub use creamui_macros as macros;
#[cfg(feature = "jsx")]
pub use creamui_macros::{abi_jsx, component, jsx};

#[cfg(feature = "abi")]
pub use creamui_abi as abi;
#[cfg(feature = "dynamic")]
pub use creamui_dynamic as dynamic;
#[cfg(feature = "ffi")]
pub use creamui_ffi as ffi;
