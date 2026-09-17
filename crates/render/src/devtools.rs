//! Optional extension point for development-only window tooling.
//!
//! `creamui-render` deliberately owns only this small integration surface.
//! Feature crates such as `creamui-devtools` register themselves before
//! [`crate::run`] and are then instantiated once for every CreamUI window.

use creamui_core::metrics::FrameMetrics;
use creamui_core::{Painter, Size};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// What one presented frame cost, stage by stage.
#[derive(Debug, Clone, Default)]
pub struct FrameReport {
    /// `"gpu"`, `"cpu"` or `"web"`.
    pub backend: &'static str,
    /// The GPU adapter in use, if any.
    pub adapter: Option<Rc<str>>,
    /// Whether widgets were rebuilt and laid out for this frame, rather than
    /// only repainted.
    pub rebuilt: bool,
    pub build: Duration,
    pub layout: Duration,
    pub record: Duration,
    /// CPU rasterization, or GPU instance preparation and command encoding.
    pub raster: Duration,
    /// Handing the frame to the compositor.
    pub present: Duration,
    pub display_items: usize,
    pub damaged_regions: usize,
    pub damaged_pixels: u64,
    pub frame_pixels: u64,
    pub cached_text_layouts: usize,
    pub cached_glyphs: usize,
    /// Engine counters for this frame — zeroed unless `creamui-core`'s
    /// `perf-metrics` feature is enabled.
    pub metrics: FrameMetrics,
}

impl FrameReport {
    pub fn total(&self) -> Duration {
        self.build + self.layout + self.record + self.raster + self.present
    }
}

/// Development tooling attached to one live CreamUI window.
///
/// Implementations are created by a [`Devtools`] factory, rather than shared
/// between windows, because frame state and visibility are window-local.
pub trait WindowDevtools {
    /// Called after every presented frame.
    fn frame_presented(&mut self, report: &FrameReport);

    /// Paints the overlay on top of the frame being recorded.
    fn paint_overlay(&self, painter: &mut dyn Painter, viewport: Size);

    /// How often the overlay needs repainting to stay current, or `None`
    /// while hidden.
    fn refresh_interval(&self) -> Option<Duration>;

    /// Handles F3 for this window. Return `true` when its visible output
    /// changed and CreamUI should schedule a paint-only repaint.
    fn toggle(&mut self) -> bool;
}

/// Factory for development tooling. Register one with [`install_devtools`]
/// before opening windows.
pub trait Devtools: 'static {
    /// Creates the development-tool state for one new window.
    fn attach_window(&self) -> Box<dyn WindowDevtools>;
}

thread_local! {
    static INSTALLED_DEVTOOLS: RefCell<Option<Rc<dyn Devtools>>> = const { RefCell::new(None) };
}

/// Registers development tooling for subsequently opened CreamUI windows.
///
/// Calling this again replaces the previous factory. This is intentionally a
/// thread-local registration: CreamUI's event loop, windows and painters are
/// all main-thread objects and no `Send` synchronization is necessary.
pub fn install_devtools(devtools: Rc<dyn Devtools>) {
    INSTALLED_DEVTOOLS.with(|installed| *installed.borrow_mut() = Some(devtools));
}

pub(crate) fn devtools_for_new_window() -> Option<Box<dyn WindowDevtools>> {
    INSTALLED_DEVTOOLS.with(|installed| {
        installed
            .borrow()
            .as_ref()
            .map(|devtools| devtools.attach_window())
    })
}
