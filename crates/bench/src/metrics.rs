//! Thin convenience layer over `creamui_core::metrics` for benchmarks.

pub use creamui_core::metrics::{frame_metrics, reset_frame_metrics, FrameMetrics};

/// Resets the engine counters, runs `f`, and returns its result plus the
/// counters accumulated while it ran.
pub fn measure<T>(f: impl FnOnce() -> T) -> (T, FrameMetrics) {
    reset_frame_metrics();
    let value = f();
    (value, frame_metrics())
}
