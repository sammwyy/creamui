//! Shared design tokens and layout helpers used across every panel.

use crate::prelude::*;

/// Sidebar categories in display order.
pub const NAV_LABELS: [&str; 17] = [
    "Appearance",
    "Typography",
    "Input",
    "Pickers",
    "Images",
    "Button",
    "Slider",
    "Checkbox",
    "Selection",
    "Feedback",
    "Sidebar",
    "Tabs",
    "Scroll",
    "Tree",
    "Table",
    "Grid",
    "Flex",
];
pub const ACCENTS: [(&str, Color); 5] = [
    ("Lilac", Color::rgb(181, 139, 255)),
    ("Sky", Color::rgb(118, 192, 255)),
    ("Mint", Color::rgb(105, 218, 166)),
    ("Berry", Color::rgb(248, 135, 181)),
    ("Apricot", Color::rgb(255, 177, 109)),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    Midnight,
}

/// Shifts each color channel by `delta`, clamping at the `u8` bounds. Used to
/// derive hover/pressed accent shades from whichever swatch is selected.
pub fn shade(color: Color, delta: i32) -> Color {
    let shift = |c: u8| (c as i32 + delta).clamp(0, 255) as u8;
    Color::rgb(shift(color.r), shift(color.g), shift(color.b))
}

pub fn accent_foreground(color: Color) -> Color {
    let luminance =
        (color.r as f32 * 0.2126 + color.g as f32 * 0.7152 + color.b as f32 * 0.0722) / 255.;
    if luminance > 0.56 {
        Color::rgb(0x2d, 0x29, 0x2b)
    } else {
        Color::rgb(0xff, 0xff, 0xff)
    }
}

pub fn build_theme(mode: ThemeMode, accent: Color) -> Theme {
    let mut theme = match mode {
        ThemeMode::Light => Theme::light(),
        ThemeMode::Dark => Theme::dark(),
        ThemeMode::Midnight => Theme::midnight(),
    };
    theme.colors.accent = accent;
    theme.colors.accent_hover = shade(accent, if mode == ThemeMode::Light { -12 } else { 20 });
    theme.colors.accent_pressed = shade(accent, -26);
    theme.colors.selection_background = accent;
    theme.colors.selection_text = accent_foreground(accent);
    theme
}

pub fn label_style() -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            // Auto so a wrapped second line grows the box instead of overflowing it.
            height: Dimension::Auto,
        },
        ..Default::default()
    }
}

/// Vertical rhythm between the cards inside a panel — wider than the
/// theme's own `spacing_large` so each topic reads as a separate block
/// instead of one continuous, undifferentiated column.
pub fn section_gap() -> f32 {
    let theme = use_theme();
    theme.spacing_large * 1.75
}

/// A section heading: a bold title plus a muted one-line description.
#[component]
pub fn SectionHeader(title: String, subtitle: String) -> BoxedWidget {
    let theme = use_theme();
    // Auto height so a wrapped title doesn't overflow into the subtitle.
    let heading_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        ..Default::default()
    };
    Box::new(jsx! {
        <RawView style={column(4.0)}>
            <Heading size={TextSize::Xl} style={heading_style}>{title}</Heading>
            <Text color={theme.text_secondary} align={TextAlign::Start} style={label_style()}>{subtitle}</Text>
        </RawView>
    })
}
