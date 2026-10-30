//! Query the host preference without changing it or loading GTK/Qt.
use creamui_theme::Color;
pub use creamui_theme::{AppearanceSource, SystemAppearance, SystemColorScheme};
use serde::Deserialize;
use std::path::PathBuf;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

/// Best-effort host query. Unsupported or unavailable providers return an
/// explicit NoPreference. Process-local CREAMUI_THEME/VARIANT overrides are
/// deliberately excluded: they describe the app, not the host.
pub fn query() -> SystemAppearance {
    merge_snapshot(query_native())
}

/// Query the operating system without the optional session snapshot. Useful
/// for a desktop provider that must avoid reading back its own published file.
pub fn query_native() -> SystemAppearance {
    #[cfg(target_os = "linux")]
    let host = linux::query();
    #[cfg(target_os = "windows")]
    let host = windows::query();
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let host = SystemAppearance::default();
    host
}

/// Source selected by the session. It is a stable pointer and may carry any
/// variant ID, rather than only the standard light/dark preference.
pub fn desktop_snapshot_path() -> Option<PathBuf> {
    std::env::var_os("CREAMUI_APPEARANCE")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[derive(Deserialize)]
struct Metadata {
    schema: u32,
    variant_id: String,
    #[serde(default)]
    color_scheme: Option<String>,
    accent: String,
    #[serde(default)]
    high_contrast: Option<bool>,
    #[serde(default)]
    reduced_motion: Option<bool>,
}

fn merge_snapshot(host: SystemAppearance) -> SystemAppearance {
    let Some(path) = desktop_snapshot_path() else {
        return host;
    };
    let Ok(source) = std::fs::read_to_string(path) else {
        return host;
    };
    merge_snapshot_source(host, &source)
}

fn merge_snapshot_source(mut host: SystemAppearance, source: &str) -> SystemAppearance {
    let Ok(snapshot) = toml::from_str::<Metadata>(source) else {
        return host;
    };
    if snapshot.schema != 1 || snapshot.variant_id.trim().is_empty() {
        return host;
    }
    host.color_scheme = snapshot
        .color_scheme
        .as_deref()
        .and_then(parse_scheme)
        .or_else(|| known_variant_scheme(&snapshot.variant_id))
        .unwrap_or(host.color_scheme);
    host.variant = Some(snapshot.variant_id);
    host.accent = snapshot.accent.parse::<Color>().ok().or(host.accent);
    host.high_contrast = snapshot.high_contrast.unwrap_or(host.high_contrast);
    host.reduced_motion = snapshot.reduced_motion.unwrap_or(host.reduced_motion);
    host.source = AppearanceSource::DesktopSnapshot;
    host
}

pub fn parse_scheme(value: &str) -> Option<SystemColorScheme> {
    match value {
        "light" => Some(SystemColorScheme::Light),
        "dark" => Some(SystemColorScheme::Dark),
        "default" | "no-preference" => Some(SystemColorScheme::NoPreference),
        _ => None,
    }
}

pub fn known_variant_scheme(id: &str) -> Option<SystemColorScheme> {
    match id {
        "light" => Some(SystemColorScheme::Light),
        "dark" | "midnight" => Some(SystemColorScheme::Dark),
        _ => None,
    }
}

/// The standard Windows guidance classifies a light foreground as a dark
/// appearance. Kept portable so the color classification is tested on Linux.
pub fn scheme_from_foreground(r: u8, g: u8, b: u8) -> SystemColorScheme {
    if 5 * u32::from(g) + 2 * u32::from(r) + u32::from(b) > 8 * 128 {
        SystemColorScheme::Dark
    } else {
        SystemColorScheme::Light
    }
}

#[cfg(target_os = "windows")]
#[doc(hidden)]
pub use windows::NativeSubscription;
#[cfg(not(target_os = "windows"))]
#[doc(hidden)]
pub struct NativeSubscription;
#[cfg(not(target_os = "windows"))]
impl NativeSubscription {
    pub fn start(_: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Option<Self> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_metadata_preserves_arbitrary_variants_and_declared_scheme() {
        let host = SystemAppearance {
            color_scheme: SystemColorScheme::Light,
            ..Default::default()
        };
        let resolved = merge_snapshot_source(
            host.clone(),
            r##"schema = 1
variant_id = "ink"
color_scheme = "dark"
accent = "#123456"
high_contrast = true
"##,
        );
        assert_eq!(resolved.variant.as_deref(), Some("ink"));
        assert_eq!(resolved.color_scheme, SystemColorScheme::Dark);
        assert_eq!(resolved.accent, Some(Color::rgb(0x12, 0x34, 0x56)));
        assert!(resolved.high_contrast);
        assert_eq!(resolved.source, AppearanceSource::DesktopSnapshot);
        assert_eq!(merge_snapshot_source(host.clone(), "schema = 99"), host);
    }
    #[test]
    fn legacy_midnight_snapshot_retains_dark_fallback_but_unknown_names_do_not_guess() {
        for (variant, scheme) in [
            ("midnight", SystemColorScheme::Dark),
            ("ink", SystemColorScheme::NoPreference),
        ] {
            let source = format!("schema = 1\nvariant_id = '{variant}'\naccent = '#123456'\n");
            let resolved = merge_snapshot_source(SystemAppearance::default(), &source);
            assert_eq!(resolved.variant.as_deref(), Some(variant));
            assert_eq!(resolved.color_scheme, scheme);
        }
    }
    #[test]
    fn windows_foreground_and_portal_values_have_the_correct_direction() {
        assert_eq!(
            scheme_from_foreground(255, 255, 255),
            SystemColorScheme::Dark
        );
        assert_eq!(scheme_from_foreground(0, 0, 0), SystemColorScheme::Light);
        assert_eq!(SystemColorScheme::from_portal(1), SystemColorScheme::Dark);
        assert_eq!(SystemColorScheme::from_portal(2), SystemColorScheme::Light);
        assert_eq!(
            SystemColorScheme::from_portal(99),
            SystemColorScheme::NoPreference
        );
    }
}
