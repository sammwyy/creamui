//! Engine counters for measuring how much reconcile/layout/paint work a
//! frame actually did. Always compiled (the thread-local itself is free),
//! but nothing increments a counter unless the `perf-metrics` feature is
//! enabled — see the call sites in `scene.rs` and `creamui-render`.

use std::cell::Cell;

/// One frame's worth of engine counters: how many nodes were reconciled,
/// laid out, and painted, and how much of that work actually touched
/// `taffy` or a rasterizer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FrameMetrics {
    pub root_builds: u64,
    pub widget_objects_built: u64,
    pub reconcile_visits: u64,

    pub taffy_style_writes: u64,
    pub taffy_context_writes: u64,
    pub taffy_children_writes: u64,
    pub layout_runs: u64,
    pub measure_calls: u64,

    pub paint_nodes_visited: u64,
    pub paint_nodes_recorded: u64,
    pub hit_nodes_updated: u64,
    pub composite_nodes_updated: u64,

    pub display_items: u64,
    pub text_layouts: u64,
    pub cpu_pixels_rasterized: u64,
    pub gpu_upload_bytes: u64,
    pub draw_calls: u64,

    pub damaged_rect_count: u64,
    pub damaged_pixel_area: u64,

    /// How many legacy `Widget` subtrees were translated into the
    /// persistent runtime tree via `runtime::mount_legacy_widget` — the
    /// old full-rebuild path, still in use until a caller migrates to
    /// mutating the runtime tree directly.
    pub legacy_widgets_mounted: u64,
}

thread_local! {
    static METRICS: Cell<FrameMetrics> = Cell::new(FrameMetrics::default());
}

/// Zeroes the thread-local counters. Call before whatever span of work
/// should be measured (e.g. once per benchmark iteration).
pub fn reset_frame_metrics() {
    METRICS.with(|m| m.set(FrameMetrics::default()));
}

/// A snapshot of the counters accumulated since the last
/// [`reset_frame_metrics`] call on this thread.
pub fn frame_metrics() -> FrameMetrics {
    METRICS.with(|m| m.get())
}

/// Mutates the current thread's counters. Only called from sites gated on
/// `#[cfg(feature = "perf-metrics")]`.
pub fn record(f: impl FnOnce(&mut FrameMetrics)) {
    METRICS.with(|m| {
        let mut metrics = m.get();
        f(&mut metrics);
        m.set(metrics);
    });
}
