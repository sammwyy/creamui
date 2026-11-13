//! The shared visual language of themed widgets.
//!
//! Surfaces are flat and quiet: accent colors are painted as solid fills,
//! neutral controls are told apart by a gentle fill and a hairline edge, and
//! shadows are soft, neutral and reserved for things that actually sit above
//! the page. Every themed widget derives its fills, edges and depth from the
//! functions here so the whole interface reads as one material.

use creamui_core::BoxShadow;
use creamui_theme::{Color, Theme};

const INK: Color = Color::rgb(16, 24, 40);
const WHITE: Color = Color::rgb(255, 255, 255);

/// How far a surface sits above the canvas it is painted on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
    /// Resting controls such as buttons and switch thumbs.
    Control,
    /// Grouped content such as cards.
    Raised,
    /// Transient surfaces: menus, popovers, tooltips and toasts.
    Overlay,
    /// Dialogs and sheets that block the page underneath.
    Modal,
}

impl Elevation {
    pub fn shadow(self, theme: &Theme) -> BoxShadow {
        let (y, blur, spread, dark_alpha, light_alpha) = match self {
            Self::Control => (1.0, 2.0, 0.0, 60, 16),
            Self::Raised => (1.0, 4.0, 0.0, 70, 18),
            Self::Overlay => (6.0, 20.0, -2.0, 110, 30),
            Self::Modal => (14.0, 40.0, -6.0, 130, 42),
        };
        BoxShadow::new(
            0.0,
            y,
            blur,
            spread,
            if is_light(theme) {
                with_alpha(INK, light_alpha)
            } else {
                Color::rgba(0, 0, 0, dark_alpha)
            },
        )
    }
}

/// True when the theme paints dark content on a light canvas.
pub fn is_light(theme: &Theme) -> bool {
    let surface = theme.colors.surface;
    (surface.r as u32 + surface.g as u32 + surface.b as u32) > 3 * 128
}

/// An opaque version of `color` composited over the theme canvas, so
/// translucent surface tokens can seed solid fills.
pub fn opaque(theme: &Theme, color: Color) -> Color {
    if color.a == 255 {
        return color;
    }
    let base = Color {
        a: 255,
        ..theme.colors.surface
    };
    let backdrop = if is_light(theme) {
        base.mix(WHITE, 0.6)
    } else {
        base
    };
    Color {
        a: 255,
        ..backdrop.mix(Color { a: 255, ..color }, color.a as f32 / 255.0)
    }
}

/// `color` with its alpha replaced.
pub const fn with_alpha(color: Color, alpha: u8) -> Color {
    Color { a: alpha, ..color }
}

/// A fully transparent color, for states that clear a fill or edge.
pub const fn clear() -> Color {
    Color::rgba(0, 0, 0, 0)
}

/// The resting fill of a neutral button-like control: white in light themes,
/// one quiet step above the page in dark ones.
pub fn control_fill(theme: &Theme) -> Color {
    if is_light(theme) {
        WHITE
    } else {
        opaque(theme, theme.colors.surface_elevated).mix(WHITE, 0.075)
    }
}

/// [`control_fill`] under the pointer.
pub fn control_hover(theme: &Theme) -> Color {
    if is_light(theme) {
        WHITE.mix(INK, 0.035)
    } else {
        control_fill(theme).mix(WHITE, 0.05)
    }
}

/// [`control_fill`] while pressed.
pub fn control_pressed(theme: &Theme) -> Color {
    if is_light(theme) {
        WHITE.mix(INK, 0.075)
    } else {
        control_fill(theme).mix(WHITE, 0.1)
    }
}

/// The hairline edge of a neutral control.
pub fn control_border(theme: &Theme) -> Color {
    if is_light(theme) {
        with_alpha(INK, 34)
    } else {
        with_alpha(WHITE, 16)
    }
}

/// The resting shadow of a neutral control. Only light themes use one: on a
/// dark canvas the lighter fill already separates the control.
pub fn control_shadow(theme: &Theme) -> Option<BoxShadow> {
    (is_light(theme) && !theme.high_contrast).then(|| Elevation::Control.shadow(theme))
}

/// A transparent wash for ghost controls, tags and list rows: `level` 1 is
/// hover, 2 is pressed or selected.
pub fn subtle(theme: &Theme, level: u8) -> Color {
    let step = if is_light(theme) { 10 } else { 13 };
    with_alpha(theme.colors.text_primary, step * level.min(3))
}

/// The resting fill of an editable field.
pub fn field_fill(theme: &Theme) -> Color {
    if is_light(theme) {
        WHITE
    } else {
        opaque(theme, theme.colors.surface).mix(WHITE, 0.05)
    }
}

/// The resting edge of an editable field.
pub fn field_border(theme: &Theme) -> Color {
    if is_light(theme) {
        with_alpha(INK, 38)
    } else {
        with_alpha(WHITE, 20)
    }
}

/// A translucent accent halo drawn outside focused controls.
pub fn focus_ring(theme: &Theme) -> Color {
    with_alpha(theme.colors.accent, 120)
}

/// Width of [`focus_ring`].
pub const FOCUS_RING_WIDTH: f32 = 2.0;

/// Applies the shared field treatment: a quiet fill and hairline edge, an
/// accent edge plus a soft halo while focused.
pub fn apply_field<W: creamui_core::Styled>(widget: W, theme: &Theme) -> W {
    let width = theme.input_border_width.max(1.0);
    let hover_edge = if is_light(theme) {
        with_alpha(INK, 60)
    } else {
        with_alpha(WHITE, 34)
    };
    widget
        .background(field_fill(theme))
        .border(field_border(theme), width)
        .corner_radius(theme.input_radius)
        .hover_style(creamui_core::StateStyle::new().border(hover_edge, width))
        .focus_style(
            creamui_core::StateStyle::new()
                .border(theme.colors.accent, width)
                .outline(with_alpha(theme.colors.accent, 60), 3.0),
        )
}

/// The recessed track behind segmented controls and tab groups.
pub fn track_fill(theme: &Theme) -> Color {
    if is_light(theme) {
        with_alpha(INK, 14)
    } else {
        Color::rgba(0, 0, 0, 56)
    }
}

/// The selected segment inside a [`track_fill`].
pub fn segment_fill(theme: &Theme) -> Color {
    if is_light(theme) {
        WHITE
    } else {
        opaque(theme, theme.colors.surface_elevated).mix(WHITE, 0.12)
    }
}

/// The unfilled part of sliders, switches and progress indicators.
pub fn rail_fill(theme: &Theme) -> Color {
    if is_light(theme) {
        with_alpha(INK, 30)
    } else {
        with_alpha(WHITE, 36)
    }
}

/// The shadow color under switch thumbs and slider handles.
pub fn knob_shadow(theme: &Theme) -> Color {
    if is_light(theme) {
        with_alpha(INK, 50)
    } else {
        Color::rgba(0, 0, 0, 110)
    }
}

/// The fill of a grouped region such as a card: white in light themes, one
/// quiet step above the panel in dark ones.
pub fn group_fill(theme: &Theme) -> Color {
    if is_light(theme) {
        WHITE
    } else {
        opaque(theme, theme.colors.surface_elevated).mix(WHITE, 0.03)
    }
}

/// The opaque fill of menus, popovers and other floating surfaces.
pub fn overlay_fill(theme: &Theme) -> Color {
    if is_light(theme) {
        WHITE
    } else {
        opaque(theme, theme.colors.surface_elevated).mix(WHITE, 0.05)
    }
}

/// The hairline edge of floating surfaces.
pub fn overlay_border(theme: &Theme) -> Color {
    if is_light(theme) {
        with_alpha(INK, 26)
    } else {
        with_alpha(WHITE, 20)
    }
}

/// The dimmer painted behind modal surfaces.
pub fn scrim(theme: &Theme) -> Color {
    if is_light(theme) {
        with_alpha(INK, 64)
    } else {
        Color::rgba(0, 0, 0, 120)
    }
}

/// A gentle wash of `color` for status backgrounds such as alerts.
pub fn tint(theme: &Theme, color: Color) -> Color {
    opaque(theme, group_fill(theme)).mix(color, if is_light(theme) { 0.1 } else { 0.14 })
}

/// A status color nudged toward the text color so it stays readable as
/// foreground over its own [`tint`].
pub fn tinted_text(theme: &Theme, color: Color) -> Color {
    if is_light(theme) {
        color.mix(Color::rgb(0, 0, 0), 0.3)
    } else {
        color.mix(WHITE, 0.3)
    }
}

/// Paints a neutral control: an optional soft shadow, its fill and edge.
pub fn paint_control(
    painter: &mut dyn creamui_core::Painter,
    theme: &Theme,
    rect: creamui_core::Rect,
    fill: Color,
    radius: f32,
) {
    if let Some(shadow) = control_shadow(theme) {
        paint_shadow(painter, theme, rect, shadow, radius);
    }
    painter.fill_rect(rect, fill, radius);
    painter.stroke_rect_inside(rect, control_border(theme), 1.0, radius);
}

/// Paints a popup trigger such as a select or picker: a neutral control
/// that gains an accent edge and halo while its popup is open.
pub fn paint_trigger(
    painter: &mut dyn creamui_core::Painter,
    theme: &Theme,
    rect: creamui_core::Rect,
    open: bool,
    interactive: bool,
) {
    let fill = if interactive && painter.pressed(rect) {
        control_pressed(theme)
    } else if interactive && painter.hovered(rect) {
        control_hover(theme)
    } else {
        control_fill(theme)
    };
    paint_control(painter, theme, rect, fill, theme.input_radius);
    if open {
        painter.stroke_rect_inside(rect, theme.colors.accent, 1.0, theme.input_radius);
        painter.stroke_rect(
            rect.inflate(3.0),
            with_alpha(theme.colors.accent, 60),
            3.0,
            theme.input_radius + 3.0,
        );
    }
}

/// Paints `shadow` under `rect`.
pub fn paint_shadow(
    painter: &mut dyn creamui_core::Painter,
    theme: &Theme,
    rect: creamui_core::Rect,
    shadow: BoxShadow,
    radius: f32,
) {
    painter.draw_box_shadow(
        rect,
        shadow.color.resolve(&theme.colors),
        shadow.offset_x,
        shadow.offset_y,
        shadow.blur,
        shadow.spread,
        radius,
    );
}
