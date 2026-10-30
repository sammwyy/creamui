//! Framework-native, already-resolved appearances supplied by a desktop.
//! No desktop-specific package formats or dependencies enter CreamUI.
use crate::{colors_from_file, ColorSchemeFile, ThemeLoadError};
use creamui_theme::{Color, CornerStyle, ResolvedAppearance, Theme};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Mutex, OnceLock},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema: u32,
    theme_id: String,
    variant_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color_scheme: Option<String>,
    accent: String,
    font_family: String,
    corners: String,
    high_contrast: bool,
    text_scale: f32,
    colors: BTreeMap<String, String>,
    style: BTreeMap<String, f32>,
    typography: BTreeMap<String, f32>,
}

macro_rules! style_fields { ($callback:ident, $($args:tt)*) => { $callback!($($args)*;
    radius_small, radius_medium, radius_large, spacing_small, spacing_medium, spacing_large,
    button_radius, checkbox_radius, input_radius, textarea_radius, input_border_width,
    card_radius, scroll_radius, tabs_radius, tab_radius, sidebar_radius, sidebar_item_radius,
    indicator_thickness, tab_gap, sidebar_gap, sidebar_icon_size, sidebar_icon_radius,
    sidebar_item_gap, menu_radius, menu_item_radius); }; }

pub fn appearance_snapshot(appearance: &ResolvedAppearance) -> Result<String, ThemeLoadError> {
    appearance_snapshot_with_scheme(
        appearance,
        creamui_appearance::known_variant_scheme(&appearance.variant_id).unwrap_or_default(),
    )
}

/// Export an arbitrary named variant with its declared standard preference.
pub fn appearance_snapshot_with_scheme(
    appearance: &ResolvedAppearance,
    scheme: creamui_theme::SystemColorScheme,
) -> Result<String, ThemeLoadError> {
    let theme = appearance.theme;
    let c = theme.colors;
    let mut colors = BTreeMap::new();
    macro_rules! color { ($($field:ident),*) => { $(colors.insert(stringify!($field).into(), hex(c.$field));)* } }
    color!(
        surface,
        surface_elevated,
        surface_hover,
        accent,
        accent_hover,
        accent_pressed,
        selection_background,
        selection_text,
        text_primary,
        text_secondary,
        text_disabled,
        border,
        border_strong,
        danger,
        warning,
        success
    );
    let mut style = BTreeMap::new();
    macro_rules! save { ($style:ident, $theme:ident; $($field:ident),*) => { $($style.insert(stringify!($field).into(), $theme.$field);)* } }
    style_fields!(save, style, theme);
    let typography = BTreeMap::from([
        ("caption".into(), theme.typography.caption),
        ("body".into(), theme.typography.body),
        ("section".into(), theme.typography.section),
        ("title".into(), theme.typography.title),
    ]);
    toml::to_string_pretty(&Snapshot {
        schema: 1,
        color_scheme: Some(scheme.as_str().into()),
        theme_id: appearance.theme_id.clone(),
        variant_id: appearance.variant_id.clone(),
        accent: hex(appearance.accent),
        font_family: appearance
            .font_family
            .clone()
            .unwrap_or_else(|| theme.font_family.into()),
        corners: appearance.corners.id().into(),
        high_contrast: theme.high_contrast,
        text_scale: theme.text_scale,
        colors,
        style,
        typography,
    })
    .map_err(|e| ThemeLoadError::InvalidThemeDefinition(e.to_string()))
}

pub fn load_appearance_snapshot(path: &Path) -> Result<ResolvedAppearance, ThemeLoadError> {
    let source = std::fs::read_to_string(path).map_err(ThemeLoadError::Io)?;
    parse_appearance_snapshot(&source)
}

pub fn parse_appearance_snapshot(source: &str) -> Result<ResolvedAppearance, ThemeLoadError> {
    let snapshot: Snapshot = toml::from_str(source).map_err(ThemeLoadError::InvalidAppearance)?;
    let invalid = |message: &str| ThemeLoadError::InvalidThemeDefinition(message.into());
    if snapshot
        .color_scheme
        .as_deref()
        .is_some_and(|s| creamui_appearance::parse_scheme(s).is_none())
    {
        return Err(invalid("invalid snapshot color scheme"));
    }
    if snapshot.schema != 1
        || !snapshot.text_scale.is_finite()
        || !(0.75..=2.0).contains(&snapshot.text_scale)
    {
        return Err(invalid("invalid appearance snapshot version or text scale"));
    }
    if snapshot.font_family.trim().is_empty() || snapshot.font_family.chars().any(char::is_control)
    {
        return Err(invalid("invalid snapshot font"));
    }
    let colors_value =
        toml::Value::try_from(&snapshot.colors).map_err(|e| invalid(&e.to_string()))?;
    let colors: ColorSchemeFile = colors_value
        .try_into()
        .map_err(ThemeLoadError::InvalidAppearance)?;
    let mut theme = Theme::default().with_colors(colors_from_file(colors)?);
    let mut style = snapshot.style;
    macro_rules! restore { ($style:ident, $theme:ident; $($field:ident),*) => { $(if let Some(value) = $style.remove(stringify!($field)) {
        if !value.is_finite() || !(0.0..=1024.0).contains(&value) { return Err(invalid(concat!("invalid snapshot style ", stringify!($field)))); }
        $theme.$field = value;
    })* } }
    style_fields!(restore, style, theme);
    if !style.is_empty() {
        return Err(invalid("unknown snapshot style token"));
    }
    for (name, value) in snapshot.typography {
        if !value.is_finite() || !(0.1..=1024.0).contains(&value) {
            return Err(invalid("invalid snapshot typography"));
        }
        match name.as_str() {
            "caption" => theme.typography.caption = value,
            "body" => theme.typography.body = value,
            "section" => theme.typography.section = value,
            "title" => theme.typography.title = value,
            _ => return Err(invalid("unknown snapshot typography token")),
        }
    }
    theme.font_family = intern_font_family(&snapshot.font_family);
    theme.high_contrast = snapshot.high_contrast;
    theme.text_scale = snapshot.text_scale;
    let accent = snapshot
        .accent
        .parse()
        .map_err(|_| invalid("invalid snapshot accent"))?;
    let corners = CornerStyle::from_id(&snapshot.corners)
        .ok_or_else(|| invalid("invalid snapshot corners"))?;
    Ok(ResolvedAppearance {
        theme_id: snapshot.theme_id,
        variant_id: snapshot.variant_id,
        accent,
        font_family: Some(snapshot.font_family),
        corners,
        theme,
    })
}

/// Theme currently stores a static family stack. Intern only distinct families,
/// so repeated reloads reuse the same allocation.
pub fn intern_font_family(family: &str) -> &'static str {
    static FAMILIES: OnceLock<Mutex<BTreeMap<String, &'static str>>> = OnceLock::new();
    let mut families = FAMILIES
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *families
        .entry(family.into())
        .or_insert_with(|| Box::leak(family.to_owned().into_boxed_str()))
}

fn hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_snapshot_preserves_geometry_fonts_and_effective_accessibility() {
        let theme = Theme {
            button_radius: 3.,
            font_family: "Test Sans",
            high_contrast: true,
            text_scale: 1.5,
            ..Theme::midnight()
        };
        let appearance = ResolvedAppearance {
            theme_id: "desktop".into(),
            variant_id: "night".into(),
            accent: theme.colors.accent,
            font_family: Some("Test Sans".into()),
            corners: CornerStyle::Soft,
            theme,
        };
        let restored =
            parse_appearance_snapshot(&appearance_snapshot(&appearance).unwrap()).unwrap();
        assert_eq!(restored, appearance);
    }
}
