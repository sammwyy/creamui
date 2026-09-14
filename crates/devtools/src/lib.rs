//! Development-only tools for CreamUI applications.
//!
//! Call [`init`] before creating CreamUI windows. Every window gets an
//! independent FPS, frame-time, CPU and memory overlay. Press F3 to show or
//! hide it.

use creamui_core::metrics::FrameMetrics;
use creamui_core::{Painter, Rect, Size, TextAlign};
use creamui_render::{install_devtools, Devtools, WindowDevtools};
use creamui_theme::Color;
use std::collections::VecDeque;
use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};
#[cfg(target_arch = "wasm32")]
use web_time::{Duration, Instant};

/// Family used for devtools text. It falls back to CreamUI's bundled font
/// unless the app registers a matching font first.
pub const DEBUG_FONT_FAMILY: &str = "CreamUI Debug, monospace";

/// Fixed corner for the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugPosition {
    #[default]
    BottomRight,
    BottomLeft,
    TopLeft,
    TopRight,
}

/// Configuration supplied to [`init_with`].
#[derive(Debug, Clone, Copy)]
pub struct DevtoolsOptions {
    /// Starts hidden by default; F3 toggles the overlay for each window.
    pub initially_visible: bool,
    pub position: DebugPosition,
}

impl Default for DevtoolsOptions {
    fn default() -> Self {
        Self {
            initially_visible: false,
            position: DebugPosition::BottomRight,
        }
    }
}

/// Installs the default devtools integration for windows opened afterwards.
///
/// Call this before [`creamui_render::run`] or [`creamui_render::AppBuilder::run`].
/// Calling it again replaces the configuration for future windows.
pub fn init() {
    init_with(DevtoolsOptions::default());
}

/// Installs devtools with explicit initial overlay options.
pub fn init_with(options: DevtoolsOptions) {
    install_devtools(Rc::new(BenchmarkDevtools { options }));
}

struct BenchmarkDevtools {
    options: DevtoolsOptions,
}

impl Devtools for BenchmarkDevtools {
    fn attach_window(&self) -> Box<dyn WindowDevtools> {
        Box::new(BenchmarkWindow {
            visible: self.options.initially_visible,
            position: self.options.position,
            frame_stats: FrameStats::new(),
            process_stats: ProcessStats::new(),
            #[cfg(feature = "perf-metrics")]
            engine_metrics: FrameMetrics::default(),
        })
    }
}

struct BenchmarkWindow {
    visible: bool,
    position: DebugPosition,
    frame_stats: FrameStats,
    process_stats: ProcessStats,
    #[cfg(feature = "perf-metrics")]
    engine_metrics: FrameMetrics,
}

impl BenchmarkWindow {
    #[cfg(feature = "perf-metrics")]
    fn engine_metrics(&self) -> Option<&FrameMetrics> {
        Some(&self.engine_metrics)
    }
    #[cfg(not(feature = "perf-metrics"))]
    fn engine_metrics(&self) -> Option<&FrameMetrics> {
        None
    }
}

impl WindowDevtools for BenchmarkWindow {
    fn after_paint(
        &mut self,
        painter: &mut dyn Painter,
        viewport: Size,
        paint_duration: Duration,
        metrics: FrameMetrics,
    ) {
        self.frame_stats.record_frame(paint_duration);
        self.process_stats.maybe_sample();
        #[cfg(feature = "perf-metrics")]
        {
            self.engine_metrics = metrics;
        }
        #[cfg(not(feature = "perf-metrics"))]
        let _ = metrics;
        if self.visible {
            draw_overlay(
                painter,
                viewport,
                self.position,
                &self.frame_stats,
                &self.process_stats,
                self.engine_metrics(),
            );
        }
    }

    fn repaint_overlay(&self, painter: &mut dyn Painter, viewport: Size) {
        if self.visible {
            draw_overlay(
                painter,
                viewport,
                self.position,
                &self.frame_stats,
                &self.process_stats,
                self.engine_metrics(),
            );
        }
    }

    fn toggle(&mut self) -> bool {
        self.visible = !self.visible;
        true
    }
}

const FRAME_HISTORY_LEN: usize = 120;

/// Rolling paint-duration and FPS tracking.
pub struct FrameStats {
    history: VecDeque<f32>,
    repaint_count: u64,
    fps: f32,
    fps_window_start: Instant,
    fps_window_frames: u32,
}

impl FrameStats {
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(FRAME_HISTORY_LEN),
            repaint_count: 0,
            fps: 0.0,
            fps_window_start: Instant::now(),
            fps_window_frames: 0,
        }
    }

    pub fn record_frame(&mut self, duration: Duration) {
        self.repaint_count += 1;
        if self.history.len() == FRAME_HISTORY_LEN {
            self.history.pop_front();
        }
        self.history.push_back(duration.as_secs_f32() * 1000.0);
        self.fps_window_frames += 1;
        let elapsed = self.fps_window_start.elapsed();
        if elapsed >= Duration::from_secs(1) {
            self.fps = self.fps_window_frames as f32 / elapsed.as_secs_f32();
            self.fps_window_frames = 0;
            self.fps_window_start = Instant::now();
        }
    }

    pub fn fps(&self) -> f32 {
        self.fps
    }
    pub fn repaint_count(&self) -> u64 {
        self.repaint_count
    }
    pub fn current_ms(&self) -> f32 {
        self.history.back().copied().unwrap_or(0.0)
    }
    pub fn min_ms(&self) -> f32 {
        if self.history.is_empty() {
            0.0
        } else {
            self.history.iter().copied().fold(f32::MAX, f32::min)
        }
    }
    pub fn max_ms(&self) -> f32 {
        self.history.iter().copied().fold(0.0, f32::max)
    }
    pub fn avg_ms(&self) -> f32 {
        if self.history.is_empty() {
            0.0
        } else {
            self.history.iter().sum::<f32>() / self.history.len() as f32
        }
    }
}

impl Default for FrameStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Process CPU/RAM sampling, resampled at most every 200ms.
pub struct ProcessStats {
    last_sample: Instant,
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    last_cpu_time: Duration,
    cpu_percent: Option<f32>,
    ram_mb: Option<f32>,
}

const MIN_SAMPLE_INTERVAL: Duration = Duration::from_millis(200);

impl ProcessStats {
    pub fn new() -> Self {
        let (cpu_time, ram_mb) = read_raw();
        Self {
            last_sample: Instant::now(),
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            last_cpu_time: cpu_time.unwrap_or(Duration::ZERO),
            cpu_percent: None,
            ram_mb,
        }
    }

    pub fn maybe_sample(&mut self) {
        let elapsed = self.last_sample.elapsed();
        if elapsed < MIN_SAMPLE_INTERVAL {
            return;
        }
        let (cpu_time, ram_mb) = read_raw();
        self.last_sample = Instant::now();
        self.ram_mb = ram_mb;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        if let Some(cpu_time) = cpu_time {
            let delta = cpu_time.saturating_sub(self.last_cpu_time);
            self.cpu_percent = Some(delta.as_secs_f32() / elapsed.as_secs_f32() * 100.0);
            self.last_cpu_time = cpu_time;
        }
    }

    pub fn cpu_percent(&self) -> Option<f32> {
        self.cpu_percent
    }
    pub fn ram_mb(&self) -> Option<f32> {
        self.ram_mb
    }
}

impl Default for ProcessStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "linux")]
fn read_raw() -> (Option<Duration>, Option<f32>) {
    (read_proc_cpu_time(), read_proc_vm_rss_mb())
}

#[cfg(target_os = "macos")]
fn read_raw() -> (Option<Duration>, Option<f32>) {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } != 0 {
        return (None, None);
    }
    let cpu_time = Duration::from_secs(usage.ru_utime.tv_sec.max(0) as u64)
        + Duration::from_micros(usage.ru_utime.tv_usec.max(0) as u64)
        + Duration::from_secs(usage.ru_stime.tv_sec.max(0) as u64)
        + Duration::from_micros(usage.ru_stime.tv_usec.max(0) as u64);
    (
        Some(cpu_time),
        Some(usage.ru_maxrss as f32 / (1024.0 * 1024.0)),
    )
}

#[cfg(target_os = "windows")]
fn read_raw() -> (Option<Duration>, Option<f32>) {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let process = unsafe { GetCurrentProcess() };
    let cpu_time =
        if unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) }
            != 0
        {
            let filetime =
                |time: FILETIME| ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64;
            Some(Duration::from_nanos(
                (filetime(kernel) + filetime(user)) * 100,
            ))
        } else {
            None
        };

    let mut memory = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    let ram_mb = (unsafe {
        GetProcessMemoryInfo(
            process,
            &mut memory,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
    } != 0)
        .then(|| memory.WorkingSetSize as f32 / (1024.0 * 1024.0));

    (cpu_time, ram_mb)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn read_raw() -> (Option<Duration>, Option<f32>) {
    (None, None)
}

#[cfg(target_os = "linux")]
fn read_proc_vm_rss_mb() -> Option<f32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let rest = line.strip_prefix("VmRSS:")?;
        let kb: f32 = rest.trim().trim_end_matches("kB").trim().parse().ok()?;
        Some(kb / 1024.0)
    })
}

#[cfg(target_os = "linux")]
fn read_proc_cpu_time() -> Option<Duration> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    let ticks_per_sec = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    (ticks_per_sec > 0)
        .then(|| Duration::from_secs_f64((utime + stime) as f64 / ticks_per_sec as f64))
}

fn format_ram(ram_mb: Option<f32>) -> String {
    ram_mb.map_or_else(|| "n/a".to_owned(), |value| format!("{value:.1} MB"))
}
fn format_cpu(cpu_percent: Option<f32>) -> String {
    cpu_percent.map_or_else(|| "n/a".to_owned(), |value| format!("{value:.1}%"))
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f32 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f32 / 1_000.0)
    } else {
        format!("{bytes} B")
    }
}

fn overlay_text(
    frame_stats: &FrameStats,
    process_stats: &ProcessStats,
    engine_metrics: Option<&FrameMetrics>,
) -> String {
    let mut text = format!(
        "FPS {:.0}\nFrame {:.1}/{:.1}/{:.1}/{:.1} ms\n(cur/avg/min/max)\nRepaints {}\nRAM {}\nCPU {}",
        frame_stats.fps(), frame_stats.current_ms(), frame_stats.avg_ms(), frame_stats.min_ms(), frame_stats.max_ms(), frame_stats.repaint_count(), format_ram(process_stats.ram_mb()), format_cpu(process_stats.cpu_percent()),
    );
    if let Some(m) = engine_metrics {
        text.push_str(&format!(
            "\n--- engine ---\nReconcile {}\nTaffy s/c/ch {}/{}/{}\nLayout {}  Measure {}\nPaint v/r {}/{}\nHit/Composite {}/{}\nDamage {} rects / {} px\nGPU {}  Draws {}",
            m.reconcile_visits,
            m.taffy_style_writes, m.taffy_context_writes, m.taffy_children_writes,
            m.layout_runs, m.measure_calls,
            m.paint_nodes_visited, m.paint_nodes_recorded,
            m.hit_nodes_updated, m.composite_nodes_updated,
            m.damaged_rect_count, m.damaged_pixel_area,
            format_bytes(m.gpu_upload_bytes), m.draw_calls,
        ));
    }
    text
}

fn draw_overlay(
    painter: &mut dyn Painter,
    viewport: Size,
    position: DebugPosition,
    frame_stats: &FrameStats,
    process_stats: &ProcessStats,
    engine_metrics: Option<&FrameMetrics>,
) {
    let text = overlay_text(frame_stats, process_stats, engine_metrics);
    let font_size = 12.0;
    let line_height = font_size * 1.5;
    let padding = 10.0;
    let width = if engine_metrics.is_some() {
        230.0
    } else {
        190.0
    };
    let height = line_height * text.lines().count().max(1) as f32 + padding * 2.0;
    let margin = 12.0;
    let (x, y) = match position {
        DebugPosition::BottomRight => (
            viewport.width - width - margin,
            viewport.height - height - margin,
        ),
        DebugPosition::BottomLeft => (margin, viewport.height - height - margin),
        DebugPosition::TopLeft => (margin, margin),
        DebugPosition::TopRight => (viewport.width - width - margin, margin),
    };
    painter.fill_rect(
        Rect {
            x,
            y,
            width,
            height,
        },
        Color::rgba(0, 0, 0, 180),
        6.0,
    );
    painter.fill_text_font(
        Rect {
            x: x + padding,
            y: y + padding,
            width: width - padding * 2.0,
            height: height - padding * 2.0,
        },
        &text,
        Color::rgb(120, 255, 140),
        font_size,
        TextAlign::Start,
        Some(DEBUG_FONT_FAMILY),
        false,
        false,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f3_toggles_visibility() {
        let mut window = BenchmarkWindow {
            visible: false,
            position: DebugPosition::default(),
            frame_stats: FrameStats::new(),
            process_stats: ProcessStats::new(),
            #[cfg(feature = "perf-metrics")]
            engine_metrics: FrameMetrics::default(),
        };
        assert!(window.toggle());
        assert!(window.visible);
        window.toggle();
        assert!(!window.visible);
    }

    #[test]
    fn frame_stats_tracks_current_min_max_avg() {
        let mut stats = FrameStats::new();
        stats.record_frame(Duration::from_millis(10));
        stats.record_frame(Duration::from_millis(20));
        stats.record_frame(Duration::from_millis(30));
        assert_eq!(stats.current_ms(), 30.0);
        assert_eq!(stats.min_ms(), 10.0);
        assert_eq!(stats.max_ms(), 30.0);
        assert!((stats.avg_ms() - 20.0).abs() < 0.01);
    }

    #[test]
    fn overlay_text_uses_real_line_breaks() {
        let text = overlay_text(&FrameStats::new(), &ProcessStats::new(), None);
        assert!(text.contains('\n'));
        assert!(!text.contains("\\n"));
    }

    #[cfg(feature = "perf-metrics")]
    #[test]
    fn overlay_text_appends_engine_counters_when_present() {
        let metrics = FrameMetrics {
            reconcile_visits: 3,
            draw_calls: 2,
            ..Default::default()
        };
        let text = overlay_text(&FrameStats::new(), &ProcessStats::new(), Some(&metrics));
        assert!(text.contains("--- engine ---"));
        assert!(text.contains("Reconcile 3"));
    }
}
