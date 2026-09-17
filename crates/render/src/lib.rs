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
pub use devtools::{install_devtools, Devtools, FrameReport, WindowDevtools};
pub use display_list::{damage, Bounds, Damage, DisplayList};
#[cfg(not(target_arch = "wasm32"))]
pub use gpu::{GpuRenderer, HeadlessGpu};
pub use raster::Rasterizer;
pub use recorder::SceneRecorder;
pub use window::{
    run, AppBuilder, AppHandle, CloseBehavior, PanicDetails, PopupOptions, WindowHandle,
    WindowOptions,
};
#[cfg(all(feature = "tray", target_os = "linux"))]
pub use window::{TrayBuilder, TrayIcon};
