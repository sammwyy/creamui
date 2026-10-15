use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use creamui_theme::{Color, Theme};
use serde::{Deserialize, Serialize};

use crate::ThemeLoadError;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AccessibilityPreferences {
    pub high_contrast: bool,
    pub text_scale: f32,
}

impl Default for AccessibilityPreferences {
    fn default() -> Self {
        Self {
            high_contrast: false,
            text_scale: 1.0,
        }
    }
}

impl AccessibilityPreferences {
    pub fn validate(self) -> Result<(), ThemeLoadError> {
        if !self.text_scale.is_finite() || !(0.75..=2.0).contains(&self.text_scale) {
            return Err(ThemeLoadError::InvalidAccessibility(
                "text_scale must be between 0.75 and 2".into(),
            ));
        }
        Ok(())
    }

    pub fn apply(self, mut theme: Theme) -> Theme {
        let ratio = self.text_scale / theme.text_scale;
        for size in [
            &mut theme.typography.caption,
            &mut theme.typography.body,
            &mut theme.typography.section,
            &mut theme.typography.title,
        ] {
            *size *= ratio;
        }
        theme.text_scale = self.text_scale;
        theme.high_contrast = self.high_contrast;
        if self.high_contrast {
            let dark = u32::from(theme.surface.r)
                + u32::from(theme.surface.g)
                + u32::from(theme.surface.b)
                < 384;
            let background = if dark {
                Color::rgb(0, 0, 0)
            } else {
                Color::rgb(255, 255, 255)
            };
            let foreground = if dark {
                Color::rgb(255, 255, 255)
            } else {
                Color::rgb(0, 0, 0)
            };
            let accent = if dark {
                Color::rgb(255, 255, 0)
            } else {
                Color::rgb(0, 0, 128)
            };
            theme.colors.surface = background;
            theme.colors.surface_elevated = background;
            theme.colors.surface_hover = if dark {
                Color::rgb(40, 40, 40)
            } else {
                Color::rgb(220, 220, 220)
            };
            theme.colors.text_primary = foreground;
            theme.colors.text_secondary = foreground;
            theme.colors.text_disabled = if dark {
                Color::rgb(160, 160, 160)
            } else {
                Color::rgb(90, 90, 90)
            };
            theme.colors.border = foreground;
            theme.colors.border_strong = foreground;
            theme.colors.accent = accent;
            theme.colors.accent_hover = accent;
            theme.colors.accent_pressed = accent;
            theme.colors.selection_background = accent;
            theme.colors.selection_text = background;
            theme.input_border_width = theme.input_border_width.max(2.0);
            theme.indicator_thickness = theme.indicator_thickness.max(3.0);
        }
        theme
    }
}

pub fn accessibility_path() -> Option<PathBuf> {
    crate::config_path().map(|path| path.with_file_name("accessibility.toml"))
}

pub fn read_system_accessibility() -> Result<AccessibilityPreferences, ThemeLoadError> {
    match accessibility_path() {
        Some(path) => read_accessibility(&path),
        None => Ok(AccessibilityPreferences::default()),
    }
}

pub fn write_system_accessibility(
    preferences: AccessibilityPreferences,
) -> Result<(), ThemeLoadError> {
    write_accessibility(
        &accessibility_path().ok_or(ThemeLoadError::ConfigurationPathUnavailable)?,
        preferences,
    )
}

pub(crate) fn read_accessibility(path: &Path) -> Result<AccessibilityPreferences, ThemeLoadError> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AccessibilityPreferences::default())
        }
        Err(error) => return Err(ThemeLoadError::Io(error)),
    };
    let preferences: AccessibilityPreferences = toml::from_str(&source)
        .map_err(|error| ThemeLoadError::InvalidAccessibility(error.to_string()))?;
    preferences.validate()?;
    Ok(preferences)
}

fn write_accessibility(
    path: &Path,
    preferences: AccessibilityPreferences,
) -> Result<(), ThemeLoadError> {
    static NEXT_WRITE: AtomicU64 = AtomicU64::new(0);
    preferences.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ThemeLoadError::Io)?;
    }
    let temporary = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        NEXT_WRITE.fetch_add(1, Ordering::Relaxed)
    ));
    let contents = toml::to_string(&preferences)
        .map_err(|error| ThemeLoadError::InvalidAccessibility(error.to_string()))?;
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(ThemeLoadError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_persist_and_invalid_writes_preserve_the_saved_selection() {
        let root = std::env::temp_dir().join(format!("cream-accessibility-{}", std::process::id()));
        let path = root.join("accessibility.toml");
        let preferences = AccessibilityPreferences {
            high_contrast: true,
            text_scale: 1.5,
        };
        write_accessibility(&path, preferences).unwrap();
        assert_eq!(read_accessibility(&path).unwrap(), preferences);
        let appearance_path = root.join("appearance.toml");
        let resolved = crate::SystemThemeLoader::new()
            .load_from_paths(Some(appearance_path.clone()), vec![])
            .unwrap();
        assert!(resolved.theme.high_contrast);
        assert_eq!(
            resolved.theme.typography.body,
            Theme::default().typography.body * 1.5
        );
        assert!(write_accessibility(
            &path,
            AccessibilityPreferences {
                text_scale: f32::NAN,
                ..preferences
            }
        )
        .is_err());
        assert_eq!(read_accessibility(&path).unwrap(), preferences);
        write_accessibility(&path, AccessibilityPreferences::default()).unwrap();
        let restored = crate::SystemThemeLoader::new()
            .load_from_paths(Some(appearance_path), vec![])
            .unwrap();
        assert!(!restored.theme.high_contrast);
        assert_eq!(restored.theme.typography, Theme::default().typography);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn high_contrast_is_opaque_and_text_scaling_is_idempotent() {
        let preferences = AccessibilityPreferences {
            high_contrast: true,
            text_scale: 1.5,
        };
        for original in [Theme::light(), Theme::dark()] {
            let theme = preferences.apply(original);
            assert_eq!(theme.text_scale, 1.5);
            assert_eq!(theme.typography.body, original.typography.body * 1.5);
            assert_eq!(theme.surface.a, 255);
            assert_eq!(theme.text_primary, theme.border);
            assert_eq!(
                u16::from(theme.surface.r).abs_diff(u16::from(theme.text_primary.r)),
                255
            );
            assert_eq!(preferences.apply(theme), theme);
        }
    }
}
