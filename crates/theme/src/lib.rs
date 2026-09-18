use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorParseError;

impl fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected #RGB, #RGBA, #RRGGBB, or #RRGGBBAA")
    }
}

impl std::error::Error for ColorParseError {}

impl FromStr for Color {
    type Err = ColorParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.strip_prefix('#').ok_or(ColorParseError)?;
        let byte =
            |index| u8::from_str_radix(&value[index..index + 2], 16).map_err(|_| ColorParseError);
        match value.len() {
            3 | 4 => {
                let component = |index| {
                    let digit = value.as_bytes()[index] as char;
                    digit
                        .to_digit(16)
                        .map(|value| (value as u8) * 17)
                        .ok_or(ColorParseError)
                };
                Ok(Self::rgba(
                    component(0)?,
                    component(1)?,
                    component(2)?,
                    if value.len() == 4 { component(3)? } else { 255 },
                ))
            }
            6 | 8 => Ok(Self::rgba(
                byte(0)?,
                byte(2)?,
                byte(4)?,
                if value.len() == 8 { byte(6)? } else { 255 },
            )),
            _ => Err(ColorParseError),
        }
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

    pub fn midnight() -> Self {
        let mut colors = Self::dark();
        colors.surface = Color::rgb(0x0a, 0x0a, 0x0c);
        colors.surface_elevated = Color::rgb(0x12, 0x11, 0x15);
        colors.surface_hover = Color::rgb(0x1d, 0x1b, 0x21);
        colors.accent = Color::rgb(0xb3, 0x8c, 0xff);
        colors.accent_hover = Color::rgb(0xc4, 0xa8, 0xff);
        colors.accent_pressed = Color::rgb(0x91, 0x69, 0xd9);
        colors.selection_background = Color::rgb(0x78, 0x56, 0xc8);
        colors.border = Color::rgb(0x29, 0x27, 0x2e);
        colors.border_strong = Color::rgb(0x43, 0x3f, 0x4a);
        colors
    }

    pub fn with_accent(mut self, accent: Color) -> Self {
        self.accent = accent;
        self.accent_hover = accent.mix(Color::rgb(255, 255, 255), 0.15);
        self.accent_pressed = accent.mix(Color::rgb(0, 0, 0), 0.15);
        self
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

    pub fn midnight() -> Self {
        Self::base().with_colors(ColorScheme::midnight())
    }

    pub fn with_accent(mut self, accent: Color) -> Self {
        self.colors = self.colors.with_accent(accent);
        self
    }

    /// Scales every radius token by the corner style's factor.
    pub fn with_corners(mut self, corners: CornerStyle) -> Self {
        let factor = corners.radius_factor();
        for radius in [
            &mut self.radius_small,
            &mut self.radius_medium,
            &mut self.radius_large,
            &mut self.button_radius,
            &mut self.checkbox_radius,
            &mut self.input_radius,
            &mut self.textarea_radius,
            &mut self.card_radius,
            &mut self.scroll_radius,
            &mut self.tabs_radius,
            &mut self.tab_radius,
            &mut self.sidebar_radius,
            &mut self.sidebar_item_radius,
            &mut self.sidebar_icon_radius,
            &mut self.menu_radius,
            &mut self.menu_item_radius,
        ] {
            *radius = (*radius * factor).round();
        }
        self
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccentPreset {
    pub id: String,
    pub name: String,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeDefinition {
    pub id: String,
    pub name: String,
    pub default_variant: String,
    pub variants: BTreeMap<String, Theme>,
    pub accent_presets: Vec<AccentPreset>,
    pub default_accent: Option<Color>,
}

impl ThemeDefinition {
    pub fn variant(&self, id: &str) -> Option<Theme> {
        self.variants.get(id).copied()
    }

    pub fn default_theme(&self) -> Theme {
        self.variant(&self.default_variant)
            .expect("theme definitions always have their default variant")
    }

    pub fn builtin_default() -> Self {
        Self {
            id: "default".into(),
            name: "Default".into(),
            default_variant: "dark".into(),
            variants: BTreeMap::from([
                ("light".into(), Theme::light()),
                ("dark".into(), Theme::dark()),
                ("midnight".into(), Theme::midnight()),
            ]),
            accent_presets: vec![
                AccentPreset {
                    id: "blue".into(),
                    name: "Blue".into(),
                    color: Color::rgb(74, 144, 226),
                },
                AccentPreset {
                    id: "cyan".into(),
                    name: "Cyan".into(),
                    color: Color::rgb(39, 215, 255),
                },
                AccentPreset {
                    id: "green".into(),
                    name: "Green".into(),
                    color: Color::rgb(61, 201, 111),
                },
                AccentPreset {
                    id: "pink".into(),
                    name: "Pink".into(),
                    color: Color::rgb(255, 117, 181),
                },
                AccentPreset {
                    id: "purple".into(),
                    name: "Purple".into(),
                    color: Color::rgb(167, 123, 255),
                },
                AccentPreset {
                    id: "orange".into(),
                    name: "Orange".into(),
                    color: Color::rgb(224, 165, 46),
                },
            ],
            default_accent: None,
        }
    }
}

/// The user's preferred corner rounding, shared by CreamUI widgets and the
/// compositor's window frames so the whole desktop agrees on one shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum CornerStyle {
    Square,
    Soft,
    #[default]
    Round,
}

impl CornerStyle {
    pub const ALL: [Self; 3] = [Self::Square, Self::Soft, Self::Round];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Square => "square",
            Self::Soft => "soft",
            Self::Round => "round",
        }
    }

    /// Case-insensitive, so hand-edited files may say `Soft` or `SOFT`.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|style| style.id().eq_ignore_ascii_case(id.trim()))
    }

    /// How much of a theme's authored radii survives: square corners drop
    /// them entirely, soft ones halve them.
    pub const fn radius_factor(self) -> f32 {
        match self {
            Self::Square => 0.,
            Self::Soft => 0.5,
            Self::Round => 1.,
        }
    }

    /// Outer corner radius for top-level windows, in logical pixels.
    pub const fn window_radius(self) -> i32 {
        match self {
            Self::Square => 0,
            Self::Soft => 6,
            Self::Round => 12,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppearanceSelection {
    pub theme: Option<String>,
    pub variant: Option<String>,
    pub accent: Option<Color>,
    /// A CSS-style family stack (e.g. `"Inter, sans-serif"`) preferred over
    /// the bundled default, resolved by loading it from the system.
    pub font_family: Option<String>,
    pub corners: Option<CornerStyle>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedAppearance {
    pub theme_id: String,
    pub variant_id: String,
    pub accent: Color,
    /// Already applied to `theme`'s radius tokens.
    pub theme: Theme,
    pub font_family: Option<String>,
    pub corners: CornerStyle,
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
    fn corner_styles_scale_radii_and_parse_case_insensitively() {
        let round = Theme::default();
        let soft = Theme::default().with_corners(CornerStyle::Soft);
        let square = Theme::default().with_corners(CornerStyle::Square);
        assert_eq!(soft.card_radius, (round.card_radius / 2.).round());
        assert_eq!(square.button_radius, 0.);
        assert_eq!(CornerStyle::from_id("SOFT"), Some(CornerStyle::Soft));
        assert_eq!(CornerStyle::from_id("pointy"), None);
    }

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

    #[test]
    fn builtin_theme_has_its_named_variants() {
        let theme = ThemeDefinition::builtin_default();
        assert_eq!(theme.variant("light").unwrap().colors, ColorScheme::light());
        assert_eq!(theme.variant("dark").unwrap().colors, ColorScheme::dark());
        assert_eq!(
            theme.variant("midnight").unwrap().colors,
            ColorScheme::midnight()
        );
    }

    #[test]
    fn parses_hex_colors() {
        assert_eq!("#f0a".parse(), Ok(Color::rgb(255, 0, 170)));
        assert_eq!("#f0a8".parse(), Ok(Color::rgba(255, 0, 170, 136)));
        assert_eq!("#ff75b5".parse(), Ok(Color::rgb(255, 117, 181)));
        assert_eq!("#27d7ff80".parse(), Ok(Color::rgba(39, 215, 255, 128)));
    }

    #[test]
    fn custom_accent_is_not_limited_to_presets() {
        let accent = Color::rgb(18, 171, 52);
        let resolved = ThemeDefinition::builtin_default()
            .default_theme()
            .with_accent(accent);
        assert_eq!(resolved.colors.accent, accent);
    }

    #[test]
    fn midnight_is_deeper_than_dark() {
        let dark = ColorScheme::dark();
        let midnight = ColorScheme::midnight();
        assert!(midnight.surface.r < dark.surface.r);
        assert!(midnight.surface_elevated.r < dark.surface_elevated.r);
    }
}
