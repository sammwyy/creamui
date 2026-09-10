//! CreamUI's widget tree, layout, and painting core.
//!
//! This crate is backend-agnostic: it defines [`Widget`], a [`Painter`]
//! trait implemented by rendering backends, and [`render_frame`], which
//! turns a widget tree into a laid-out, painted [`Scene`] using `taffy` for
//! CSS-like flex/grid layout.

mod geometry;
mod scene;
mod widget;

pub use geometry::{Point, Rect, Size};
pub use scene::{render_frame, Renderer, Scene};
pub use widget::{
    BoxedWidget, CursorIcon, Key, KeyInput, MeasureFn, Modifiers, Painter, TextAlign, Widget,
    WindowDragHandle,
};

/// Re-exported so downstream crates can build `taffy::style::Style` values
/// without adding a direct `taffy` dependency of their own.
pub mod layout {
    pub use taffy::prelude::*;
}
