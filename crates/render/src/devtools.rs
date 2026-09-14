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

/// Development tooling attached to one live CreamUI window.
///
/// Implementations are created by a [`Devtools`] factory, rather than shared
/// between windows, because frame state and visibility are window-local.
pub trait WindowDevtools {
    /// Called after a full layout and paint pass, while the frame's painter is
    /// still available. `paint_duration` covers the application UI only.
    /// `metrics` is the engine counter snapshot for this frame — zeroed
    /// unless `creamui-core`'s `perf-metrics` feature is enabled.
    fn after_paint(
        &mut self,
        painter: &mut dyn Painter,
        viewport: Size,
        paint_duration: Duration,
        metrics: FrameMetrics,
    );

    /// Called after a paint-only pass so tools can restore anything the pass
    /// cleared without doing another layout.
    fn repaint_overlay(&self, painter: &mut dyn Painter, viewport: Size);

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
