//! Theme primitives split deliberately in two: [`ColorScheme`] owns colours,
//! while [`Theme`] owns the shape and behaviour of components.

/// An 8-bit sRGB color with alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}
impl Color {
    /// Blend in sRGB for subtle surface and interaction treatments.
    pub fn mix(self, other: Self, amount: f32) -> Self {
        let t = amount.clamp(0., 1.);
        let c = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Self::rgba(
            c(self.r, other.r),
            c(self.g, other.g),
            c(self.b, other.b),
            c(self.a, other.a),
        )
    }
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    pub fn to_f32(self) -> [f32; 4] {
        [
            self.r as f32 / 255.,
            self.g as f32 / 255.,
            self.b as f32 / 255.,
            self.a as f32 / 255.,
        ]
    }
}

/// All colour tokens. This can be changed independently from a [`Theme`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorScheme {
    /// Background primary: the app canvas.
    pub surface: Color,
    /// Background secondary: panels, cards and resting controls.
    pub surface_elevated: Color,
    pub surface_hover: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_pressed: Color,
    pub selection_background: Color,
    pub selection_text: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_disabled: Color,
    pub border: Color,
    pub border_strong: Color,
    pub danger: Color,
    pub warning: Color,
    pub success: Color,
}
impl ColorScheme {
    /// Semantic alias for [`ColorScheme::surface`].
    ///
    /// `surface` remains the stored field for source and ABI compatibility.
    pub const fn background_primary(&self) -> Color {
        self.surface
    }

    /// Semantic alias for [`ColorScheme::surface_elevated`].
    pub const fn background_secondary(&self) -> Color {
        self.surface_elevated
    }

    pub const fn dark() -> Self {
        Self {
            // Warm charcoal rather than a blue-black: it gives vivid accents
            // room to glow without tinting the whole application purple.
            surface: Color::rgb(0x1c, 0x1b, 0x1d),
            surface_elevated: Color::rgb(0x27, 0x25, 0x27),
            surface_hover: Color::rgb(0x34, 0x31, 0x34),
            accent: Color::rgb(0xa7, 0x7b, 0xff),
            accent_hover: Color::rgb(0xb7, 0x93, 0xff),
            accent_pressed: Color::rgb(0x8d, 0x62, 0xdb),
            selection_background: Color::rgb(0x0a, 0x84, 0xff),
            selection_text: Color::rgb(0xff, 0xff, 0xff),
            text_primary: Color::rgb(0xf4, 0xf1, 0xf0),
            text_secondary: Color::rgb(0xb9, 0xb2, 0xb4),
            text_disabled: Color::rgb(0x80, 0x7a, 0x7c),
            border: Color::rgb(0x3d, 0x39, 0x3d),
            border_strong: Color::rgb(0x5d, 0x57, 0x5c),
            danger: Color::rgb(0xe5, 0x4b, 0x4b),
            warning: Color::rgb(0xe0, 0xa5, 0x2e),
            success: Color::rgb(0x3d, 0xc9, 0x6f),
        }
    }
    pub const fn light() -> Self {
        Self {
            // A light warm-grey canvas lets white secondary surfaces read as
            // deliberately layered instead of clinical.
            surface: Color::rgb(0xf5, 0xf2, 0xf0),
            surface_elevated: Color::rgb(0xff, 0xfd, 0xfc),
            surface_hover: Color::rgb(0xeb, 0xe6, 0xe5),
            accent: Color::rgb(0x9a, 0x6d, 0xf2),
            accent_hover: Color::rgb(0x88, 0x59, 0xe2),
            accent_pressed: Color::rgb(0x76, 0x48, 0xc8),
            selection_background: Color::rgb(0x0a, 0x66, 0xcc),
            selection_text: Color::rgb(0xff, 0xff, 0xff),
            text_primary: Color::rgb(0x2d, 0x29, 0x2b),
            text_secondary: Color::rgb(0x6d, 0x65, 0x69),
            text_disabled: Color::rgb(0x9b, 0x92, 0x96),
            border: Color::rgb(0xe4, 0xdd, 0xdd),
            border_strong: Color::rgb(0xc7, 0xbd, 0xbf),
            danger: Color::rgb(0xd1, 0x3a, 0x3a),
            warning: Color::rgb(0xb8, 0x7d, 0x0a),
            success: Color::rgb(0x22, 0xa0, 0x55),
        }
    }
}
impl Default for ColorScheme {
    fn default() -> Self {
        Self::dark()
    }
}

/// How a selected tab or sidebar item communicates selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStyle {
    Filled,
    Indicator,
}

/// Shared UI type roles, expressed in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Typography {
    pub caption: f32,
    pub body: f32,
    pub section: f32,
    pub title: f32,
}
impl Typography {
    pub const DEFAULT: Self = Self {
        caption: 11.,
        body: 13.,
        section: 15.,
        title: 26.,
    };
}

/// Component geometry and interaction-style metadata. It intentionally has
/// no colours, so the same style works with every accent and light/dark mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub typography: Typography,
    /// CSS-style default family stack (e.g. `"Inter, sans-serif"`) themed
    /// text widgets resolve against unless overridden per-widget.
    pub font_family: &'static str,
    /// Kept as a nested value for ergonomic backwards compatibility. New
    /// code should keep and swap a `ColorScheme` independently (or use the
    /// two providers); style tokens below never encode a colour decision.
    pub colors: ColorScheme,
    pub name: &'static str,
    pub radius_small: f32,
    pub radius_medium: f32,
    pub radius_large: f32,
    pub spacing_small: f32,
    pub spacing_medium: f32,
    pub spacing_large: f32,
    pub button_radius: f32,
    pub checkbox_radius: f32,
    pub input_radius: f32,
    pub textarea_radius: f32,
    pub input_border_width: f32,
    pub card_radius: f32,
    pub scroll_radius: f32,
    pub tabs_radius: f32,
    pub tab_radius: f32,
    pub tab_selection: SelectionStyle,
    pub sidebar_radius: f32,
    pub sidebar_item_radius: f32,
    pub sidebar_selection: SelectionStyle,
    pub indicator_thickness: f32,
    pub tab_gap: f32,
    pub sidebar_gap: f32,
    pub sidebar_icon_size: f32,
    pub sidebar_icon_radius: f32,
    pub sidebar_item_gap: f32,
    pub menu_radius: f32,
    pub menu_item_radius: f32,
}
impl Theme {
    const fn base() -> Self {
        Self {
            typography: Typography::DEFAULT,
            font_family: creamui_fonts::DEFAULT_FAMILY,
            colors: ColorScheme::dark(),
            name: "Default",
            radius_small: 8.,
            radius_medium: 12.,
            radius_large: 20.,
            spacing_small: 4.,
            spacing_medium: 8.,
            spacing_large: 16.,
            button_radius: 7.,
            checkbox_radius: 4.,
            input_radius: 7.,
            textarea_radius: 12.,
            input_border_width: 1.,
            card_radius: 12.,
            scroll_radius: 16.,
            tabs_radius: 8.,
            tab_radius: 6.,
            tab_selection: SelectionStyle::Filled,
            sidebar_radius: 16.,
            sidebar_item_radius: 7.,
            sidebar_selection: SelectionStyle::Filled,
            indicator_thickness: 3.,
            tab_gap: 4.,
            sidebar_gap: 6.,
            sidebar_icon_size: 16.,
            sidebar_icon_radius: 5.,
            sidebar_item_gap: 9.,
            menu_radius: 10.,
            menu_item_radius: 7.,
        }
    }
    pub const fn with_colors(mut self, colors: ColorScheme) -> Self {
        self.colors = colors;
        self
    }
    pub const fn dark() -> Self {
        Self::base().with_colors(ColorScheme::dark())
    }
    pub const fn light() -> Self {
        Self::base().with_colors(ColorScheme::light())
    }
}
impl std::ops::Deref for Theme {
    type Target = ColorScheme;
    fn deref(&self) -> &Self::Target {
        &self.colors
    }
}
impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

/// Reactive provider for a style theme.
#[derive(Clone)]
pub struct ThemeProvider {
    theme: creamui_reactive::Signal<Theme>,
}
impl ThemeProvider {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme: creamui_reactive::Signal::new(theme),
        }
    }
    pub fn get(&self) -> Theme {
        self.theme.get()
    }
    pub fn set(&self, theme: Theme) {
        self.theme.set(theme)
    }
}
impl Default for ThemeProvider {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}

/// Reads the current window's theme. Only callable while a
/// `creamui_reactive::with_context_scope` is active (e.g. during a window's
/// `build_ui`); panics otherwise.
pub fn use_theme() -> Theme {
    creamui_reactive::use_context::<ThemeProvider>().get()
}

/// Reactive provider for an independently configurable colour scheme.
#[derive(Clone)]
pub struct ColorSchemeProvider {
    colors: creamui_reactive::Signal<ColorScheme>,
}
impl ColorSchemeProvider {
    pub fn new(colors: ColorScheme) -> Self {
        Self {
            colors: creamui_reactive::Signal::new(colors),
        }
    }
    pub fn get(&self) -> ColorScheme {
        self.colors.get()
    }
    pub fn set(&self, colors: ColorScheme) {
        self.colors.set(colors)
    }
}
impl Default for ColorSchemeProvider {
    fn default() -> Self {
        Self::new(ColorScheme::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_style_is_rounded() {
        let theme = Theme::default();
        assert_eq!(theme.name, "Default");
        assert_eq!(theme.tab_selection, SelectionStyle::Filled);
        assert_eq!(theme.sidebar_selection, SelectionStyle::Filled);
        assert!(theme.input_radius > 0.0);
    }

    #[test]
    fn palette_can_change_without_changing_style() {
        let dark = Theme::default();
        let light = dark.with_colors(ColorScheme::light());
        assert_eq!(dark.name, light.name);
        assert_eq!(dark.tab_radius, light.tab_radius);
        assert_ne!(dark.colors, light.colors);
    }
}
