//! CreamUI's widget tree, layout, and painting core.
//!
//! This crate is backend-agnostic: it defines [`Widget`], a [`Painter`]
//! trait implemented by rendering backends, and [`render_frame`], which
//! turns a widget tree into a laid-out, painted [`Scene`] using `taffy` for
//! CSS-like flex/grid layout.

mod damage;
mod geometry;
mod image;
pub mod metrics;
pub mod runtime;
mod scene;
mod style;
mod virtualize;
mod widget;

pub use damage::{merge_damage, merge_damage_default, DEFAULT_AREA_RATIO, DEFAULT_MAX_RECTS};
pub use geometry::{Point, Rect, Size};
pub use image::RgbaImage;
pub use scene::{render_frame, Renderer, Scene};
pub use style::{
    Background, Border, BoxShadow, ColorToken, ColorValue, InteractionState, InteractionStyles,
    LengthValue, LinearGradient, PaintStyle, ResolvedStyle, StateStyle, Style, StyleParseError,
    StyleProp, StyleState, TypographyStyle,
};
pub use virtualize::{visible_range, HeightIndex};
pub use widget::{
    BoxedWidget, CursorIcon, Key, KeyInput, MeasureFn, Modifiers, Painter, Styled, TextAlign,
    Widget, WidgetKey, WindowDragHandle,
};

/// Taffy's layout-only primitives. [`crate::Style`] is CreamUI's common
/// component style; this module remains available as a migration adapter for
/// existing layout declarations and for advanced grid/flex configuration.
pub mod layout {
    pub use taffy::prelude::*;
}
