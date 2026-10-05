//! Rendering backend for CreamUI.
//!
//! Widgets paint into a [`SceneRecorder`], which records a backend-neutral
//! [`DisplayList`]. Consecutive lists are diffed into [`Damage`]; the GPU
//! backend ([`GpuRenderer`]) draws the list with one instanced SDF
//! pipeline, while the software backend ([`Rasterizer`]) replays only the
//! damaged regions with `tiny-skia` and presents just those pixels.

mod backend;
#[cfg(not(target_arch = "wasm32"))]
mod cpu;
mod devtools;
mod display_list;
#[cfg(not(target_arch = "wasm32"))]
mod gpu;
mod raster;
mod recorder;
mod text;
#[cfg(target_arch = "wasm32")]
mod web;
mod window;

pub use backend::RenderBackend;
pub use creamui_platform as platform;
#[cfg(all(feature = "platform-android", target_os = "android"))]
pub use creamui_platform::AndroidApp;
pub use creamui_platform::BlurRegion;
pub use creamui_platform::{
    CompositorControls, CompositorIntegrationMode, CompositorIntegrationRequest,
};
pub use devtools::{install_devtools, Devtools, DevtoolsCommand, FrameReport, WindowDevtools};
pub use display_list::{damage, diff, Bounds, Damage, DisplayList, FrameDiff, ScrollBlit};
#[cfg(not(target_arch = "wasm32"))]
pub use gpu::{GpuRenderer, HeadlessGpu};
pub use raster::Rasterizer;
pub use recorder::SceneRecorder;
#[cfg(all(feature = "platform-android", target_os = "android"))]
pub use window::run_android;
pub use window::{
    run, use_safe_area, use_screen_class, use_system_bars, use_viewport, AppBuilder, AppHandle,
    CloseBehavior, PanicDetails, PopupOptions, SafeArea, ScreenClass, WindowContent, WindowHandle,
    WindowOptions,
};
#[cfg(all(feature = "tray", target_os = "linux"))]
pub use window::{TrayBuilder, TrayIcon};
