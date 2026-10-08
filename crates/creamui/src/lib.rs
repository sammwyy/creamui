//! The main entry point for CreamUI applications.
//!
//! The default feature set includes the native runtime: widgets, theming,
//! reactivity, layout, and window rendering. Enable optional integrations as
//! needed, such as `image`, `jsx`, `dynamic`, `ffi`, or `devtools`.

pub use creamui_core as core;
pub use creamui_fonts as fonts;
pub use creamui_platform as platform;
pub use creamui_reactive as reactive;
pub use creamui_render as render;
pub use creamui_theme as theme;
pub use creamui_widgets as widgets;

pub use creamui_core::runtime::{IntoView, MountCx, MountedView, View};
pub use creamui_core::{
    Background, Border, BoxShadow, BoxedWidget, ColorToken, ColorValue, Easing, EdgeValues,
    InteractionState, LengthValue, LinearGradient, PaintStyle, Painter, RadialColorStop,
    RadialGradient, RadialGradientShape, RadialGradientSize, ResolvedRadialColorStop, Size,
    StateStyle, Style, StyleParseError, StyleProp, StyleState, Styled, Transition, TransitionState,
    TypographyStyle, Widget,
};
#[cfg(feature = "devtools")]
pub use creamui_devtools as devtools;
pub use creamui_fonts::{include_font, use_font, FontHandle, FontWeight};
pub use creamui_reactive::{create_effect, Effect, Signal};
pub use creamui_render::{
    run, use_safe_area, use_screen_class, use_system_bars, use_viewport, AppBuilder, AppHandle,
    BlurRegion, CloseBehavior, CompositorControls, CompositorIntegrationMode,
    CompositorIntegrationRequest, PanicDetails, RenderBackend, SafeArea, ScreenClass,
    WindowContent, WindowHandle, WindowOptions,
};
#[cfg(all(feature = "platform-android", target_os = "android"))]
pub use creamui_render::{run_android, AndroidApp};
#[cfg(all(feature = "tray", target_os = "linux"))]
pub use creamui_render::{TrayBuilder, TrayIcon};
pub use creamui_theme::{
    use_theme, AccentPreset, AppearanceSelection, Color, ColorScheme, ResolvedAppearance, Theme,
    ThemeDefinition, ThemeProvider, Typography,
};
#[cfg(feature = "system-theme")]
pub use creamui_theme_loader as theme_loader;
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
