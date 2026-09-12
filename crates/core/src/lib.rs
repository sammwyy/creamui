//! CreamUI's widget tree, layout, and painting core.
//!
//! This crate is backend-agnostic: it defines [`Widget`], a [`Painter`]
//! trait implemented by rendering backends, and [`render_frame`], which
//! turns a widget tree into a laid-out, painted [`Scene`] using `taffy` for
//! CSS-like flex/grid layout.

mod geometry;
mod scene;
mod style;
mod widget;

pub use geometry::{Point, Rect, Size};
pub use scene::{diag_take_resolve_stats, render_frame, Renderer, Scene};
pub use style::{
    Border, ColorToken, ColorValue, InteractionState, InteractionStyles, LengthValue, PaintStyle,
    ResolvedStyle, StateStyle, Style, StyleParseError, StyleProp, StyleState, TypographyStyle,
};
pub use widget::{
    BoxedWidget, CursorIcon, Key, KeyInput, MeasureFn, Modifiers, Painter, Styled, TextAlign,
    Widget, WindowDragHandle,
};

/// Taffy's layout-only primitives. [`crate::Style`] is CreamUI's common
/// component style; this module remains available as a migration adapter for
/// existing layout declarations and for advanced grid/flex configuration.
pub mod layout {
    pub use taffy::prelude::*;
}
