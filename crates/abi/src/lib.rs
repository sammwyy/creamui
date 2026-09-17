//! Plain `#[repr(C)]` ABI types and wire-format constants shared by
//! `creamui-ffi` (the producer, compiled into the `cdylib`) and
//! `creamui-dynamic` (the consumer, which `dlopen`s that `cdylib`).
//!
//! This crate has no dependency on any CreamUI engine crate (core, render,
//! theme, widgets) — depending on it, from either side of the ABI boundary,
//! never pulls in the renderer/layout engine as a compile-time dependency.
//! That matters most for `creamui-dynamic`: an app using it stays as free
//! of the engine as one talking to the C ABI directly would be, while both
//! sides of the boundary still share one definition of every struct layout
//! and wire-format constant instead of hand-copying them and risking drift.

use std::os::raw::{c_char, c_int};

/// A color: identical layout to `creamui_theme::Color`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl CColor {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        CColor { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        CColor { r, g, b, a }
    }
}

/// A full theme-token set: identical field-for-field to `creamui_theme::Theme`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CTheme {
    pub surface: CColor,
    pub surface_elevated: CColor,
    pub surface_hover: CColor,

    pub accent: CColor,
    pub accent_hover: CColor,
    pub accent_pressed: CColor,
    pub selection_background: CColor,
    pub selection_text: CColor,

    pub text_primary: CColor,
    pub text_secondary: CColor,
    pub text_disabled: CColor,

    pub border: CColor,
    pub border_strong: CColor,

    pub danger: CColor,
    pub warning: CColor,
    pub success: CColor,

    pub radius_small: f32,
    pub radius_medium: f32,
    pub radius_large: f32,

    pub spacing_small: f32,
    pub spacing_medium: f32,
    pub spacing_large: f32,
}

/// A length, tagged by `kind`: [`DIMENSION_AUTO`] (only meaningful for
/// `size`/`min_size`/`max_size`/`flex_basis` fields — treated as
/// zero-length elsewhere), [`DIMENSION_LENGTH`] (an absolute length in
/// logical pixels, in `value`), or [`DIMENSION_PERCENT`] (a percentage of
/// the containing block in the `[0.0, 1.0]` range, in `value`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CDimension {
    pub kind: u8,
    pub value: f32,
}

pub const DIMENSION_AUTO: u8 = 0;
pub const DIMENSION_LENGTH: u8 = 1;
pub const DIMENSION_PERCENT: u8 = 2;

impl CDimension {
    pub const AUTO: CDimension = CDimension {
        kind: DIMENSION_AUTO,
        value: 0.0,
    };

    pub const fn length(value: f32) -> CDimension {
        CDimension {
            kind: DIMENSION_LENGTH,
            value,
        }
    }

    pub const fn percent(value: f32) -> CDimension {
        CDimension {
            kind: DIMENSION_PERCENT,
            value,
        }
    }
}

/// Sentinel for [`CStyle::justify_content`]/[`CStyle::align_items`] meaning
/// "unset" (`None`), distinct from any real alignment value.
pub const ALIGN_UNSET: u8 = 255;

pub const JUSTIFY_START: u8 = 0;
pub const JUSTIFY_END: u8 = 1;
pub const JUSTIFY_FLEX_START: u8 = 2;
pub const JUSTIFY_FLEX_END: u8 = 3;
pub const JUSTIFY_CENTER: u8 = 4;
pub const JUSTIFY_STRETCH: u8 = 5;
pub const JUSTIFY_SPACE_BETWEEN: u8 = 6;
pub const JUSTIFY_SPACE_AROUND: u8 = 7;
pub const JUSTIFY_SPACE_EVENLY: u8 = 8;

pub const ALIGN_START: u8 = 0;
pub const ALIGN_END: u8 = 1;
pub const ALIGN_FLEX_START: u8 = 2;
pub const ALIGN_FLEX_END: u8 = 3;
pub const ALIGN_CENTER: u8 = 4;
pub const ALIGN_STRETCH: u8 = 5;
pub const ALIGN_BASELINE: u8 = 9;

pub const FLEX_DIRECTION_ROW: u8 = 0;
pub const FLEX_DIRECTION_COLUMN: u8 = 1;
pub const FLEX_DIRECTION_ROW_REVERSE: u8 = 2;
pub const FLEX_DIRECTION_COLUMN_REVERSE: u8 = 3;

/// Full flex-layout style control — the field-for-field subset of
/// `taffy::Style` (via `creamui_core::layout::Style`) that CreamUI's widgets
/// actually use. Build one with [`CStyle::default_style`] (which matches
/// `Style::default()`) and override only the fields needed.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CStyle {
    /// One of the `FLEX_DIRECTION_*` constants.
    pub flex_direction: u8,
    /// One of the `JUSTIFY_*` constants, or [`ALIGN_UNSET`] for "unset".
    pub justify_content: u8,
    /// One of the `ALIGN_*` constants, or [`ALIGN_UNSET`] for "unset".
    pub align_items: u8,
    pub width: CDimension,
    pub height: CDimension,
    pub min_width: CDimension,
    pub min_height: CDimension,
    pub max_width: CDimension,
    pub max_height: CDimension,
    pub padding_left: f32,
    pub padding_right: f32,
    pub padding_top: f32,
    pub padding_bottom: f32,
    /// `kind` [`DIMENSION_AUTO`] collapses to `taffy`'s `Auto` margin.
    pub margin_left: CDimension,
    pub margin_right: CDimension,
    pub margin_top: CDimension,
    pub margin_bottom: CDimension,
    pub gap_row: f32,
    pub gap_column: f32,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: CDimension,
}

impl CStyle {
    /// Matches `taffy::Style::default()` / `creamui_core::layout::Style::default()`:
    /// row direction, no forced alignment, auto size, zero margin/padding/gap,
    /// `flex_grow: 0`, `flex_shrink: 1`, `flex_basis: auto`.
    ///
    /// Margin is zero (not auto) to match `taffy::Style::default()` — an
    /// auto margin on the main axis acts as a flexible spacer that absorbs
    /// leftover flex space, which would silently defeat a parent's
    /// `gap`/`justify_content` for any child using this default unmodified.
    pub fn default_style() -> Self {
        CStyle {
            flex_direction: FLEX_DIRECTION_ROW,
            justify_content: ALIGN_UNSET,
            align_items: ALIGN_UNSET,
            width: CDimension::AUTO,
            height: CDimension::AUTO,
            min_width: CDimension::AUTO,
            min_height: CDimension::AUTO,
            max_width: CDimension::AUTO,
            max_height: CDimension::AUTO,
            padding_left: 0.0,
            padding_right: 0.0,
            padding_top: 0.0,
            padding_bottom: 0.0,
            margin_left: CDimension::length(0.0),
            margin_right: CDimension::length(0.0),
            margin_top: CDimension::length(0.0),
            margin_bottom: CDimension::length(0.0),
            gap_row: 0.0,
            gap_column: 0.0,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: CDimension::AUTO,
        }
    }

    /// Matches `creamui_widgets::layout::row`: a flex row with a fixed gap.
    pub fn row(gap: f32) -> Self {
        CStyle {
            flex_direction: FLEX_DIRECTION_ROW,
            gap_row: gap,
            gap_column: gap,
            ..Self::default_style()
        }
    }
}

impl Default for CStyle {
    fn default() -> Self {
        Self::default_style()
    }
}

/// One of the `TEXT_ALIGN_*` constants, or [`TEXT_ALIGN_UNSET`] for "unset".
pub const TEXT_ALIGN_CENTER: u8 = 0;
pub const TEXT_ALIGN_START: u8 = 1;
pub const TEXT_ALIGN_END: u8 = 2;
/// Sentinel for [`CTypographyStyle::align`] meaning "unset" (`None`).
pub const TEXT_ALIGN_UNSET: u8 = 255;

/// A three-state flag: [`TRISTATE_UNSET`] (`None`), [`TRISTATE_FALSE`], or
/// [`TRISTATE_TRUE`] — used by [`CTypographyStyle`]'s boolean fields, each
/// of which is independently optional on the Rust side.
pub const TRISTATE_UNSET: u8 = 0;
pub const TRISTATE_FALSE: u8 = 1;
pub const TRISTATE_TRUE: u8 = 2;

/// Typography style control — the field-for-field subset of
/// `creamui_core::TypographyStyle` that's C-representable. Every field is
/// independently optional (`has_color`/a negative `font_size`/a null
/// `font_family`/[`TEXT_ALIGN_UNSET`]/[`TRISTATE_UNSET`] mean "unset",
/// matching `TypographyStyle`'s `Option<T>` fields), so a caller can
/// override just the fields it cares about. Build one with
/// [`CTypographyStyle::unset`] and override only what's needed. `color`
/// is always a literal color, never a theme token — same restriction
/// [`CStyle`]'s color-bearing mutations already have.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CTypographyStyle {
    pub has_color: c_int,
    pub color: CColor,
    /// Negative means "unset".
    pub font_size: f32,
    /// Null means "unset". Must be a valid NUL-terminated UTF-8 string for
    /// the duration of the call it's passed to.
    pub font_family: *const c_char,
    /// One of the `TEXT_ALIGN_*` constants, or [`TEXT_ALIGN_UNSET`].
    pub align: u8,
    /// One of the `TRISTATE_*` constants.
    pub bold: u8,
    pub italic: u8,
    pub underline: u8,
    pub strikethrough: u8,
}

impl CTypographyStyle {
    /// Every field unset — start here and override only what's needed.
    pub fn unset() -> Self {
        CTypographyStyle {
            has_color: 0,
            color: CColor::rgb(0, 0, 0),
            font_size: -1.0,
            font_family: std::ptr::null(),
            align: TEXT_ALIGN_UNSET,
            bold: TRISTATE_UNSET,
            italic: TRISTATE_UNSET,
            underline: TRISTATE_UNSET,
            strikethrough: TRISTATE_UNSET,
        }
    }
}

impl Default for CTypographyStyle {
    fn default() -> Self {
        Self::unset()
    }
}

/// Render backend requested via [`CWindowOptions::backend`]: [`CUI_RENDER_BACKEND_GPU`]
/// (`wgpu`, the default) or [`CUI_RENDER_BACKEND_CPU`] (software rasterizer). Can
/// still be force-overridden at launch with `CUI_OVERRIDE_RENDER_BACKEND=gpu|cpu`.
pub const CUI_RENDER_BACKEND_GPU: c_int = 0;
pub const CUI_RENDER_BACKEND_CPU: c_int = 1;

/// A node's window-space rect as of the last computed layout: identical
/// layout to `creamui_core::Rect`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Opaque node handle: an index+generation pair packed into one `u64`
/// (index in the high 32 bits, generation in the low 32) rather than a
/// pointer, so a stale handle is safely detectable instead of aliasing
/// whatever now occupies that slot.
pub type CNode = u64;

/// Sentinel [`CNode`] meaning "no node" (e.g. no `before` sibling, no root).
pub const CUI_NODE_NONE: CNode = u64::MAX;

pub const CUI_NODE_KIND_CONTAINER: c_int = 0;
pub const CUI_NODE_KIND_TEXT: c_int = 1;

/// Window creation options. `title` must be a valid NUL-terminated UTF-8
/// string for the duration of the call it's passed to.
#[repr(C)]
pub struct CWindowOptions {
    pub title: *const c_char,
    pub width: u32,
    pub height: u32,
    pub resizable: c_int,
    pub decorations: c_int,
    pub transparent: c_int,
    /// One of [`CUI_RENDER_BACKEND_GPU`] / [`CUI_RENDER_BACKEND_CPU`].
    pub backend: c_int,
}
