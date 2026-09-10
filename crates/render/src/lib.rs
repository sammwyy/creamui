//! wgpu + winit windowing and rendering backend for CreamUI.
//!
//! Rasterization happens on the CPU via `tiny-skia` ([`painter::SkiaPainter`]);
//! the GPU ([`gpu`]) only uploads and composites the result. This keeps the
//! MVP's rendering code simple while still presenting through the GPU.

mod backend;
#[cfg(not(target_arch = "wasm32"))]
mod cpu;
mod devtools;
mod font;
#[cfg(not(target_arch = "wasm32"))]
mod gpu;
mod painter;
#[cfg(target_arch = "wasm32")]
mod web;
mod window;

pub use backend::RenderBackend;
pub use devtools::{install_devtools, Devtools, WindowDevtools};
pub use painter::SkiaPainter;
pub use window::{
    run, AppBuilder, AppHandle, CloseBehavior, PanicDetails, WindowHandle, WindowOptions,
};
#[cfg(all(feature = "tray", target_os = "linux"))]
pub use window::{TrayBuilder, TrayIcon};
