//! Loads the color theme shared by every CreamUI app from
//! `<config dir>/cream/`, so switching the active theme once applies to all
//! of them. `active_theme.toml` names the active theme id as plain text
//! (not a filesystem symlink) so this works unprivileged on Windows too.

use crate::{ColorScheme, Theme};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const DEFAULT_THEME_ID: &str = "default";

#[derive(Serialize, Deserialize)]
struct ThemeFile {
    colors: ColorScheme,
}

#[derive(Serialize, Deserialize)]
struct ActiveTheme {
    theme: String,
}

/// The active theme's colors, layered onto [`Theme::default`]'s style. On
/// first run (no `active_theme.toml` yet) this bootstraps `cream/` from
/// CreamUI's own built-in default and returns it; if the files exist but
/// are missing or malformed, it falls back to that same default in memory
/// without touching disk.
pub fn active_theme() -> Theme {
    let dir = cream_dir();
    match read_active_id(&dir) {
        Some(id) => load_theme(&dir, &id).unwrap_or_default(),
        None => {
            bootstrap(&dir);
            Theme::default()
        }
    }
}

fn read_active_id(dir: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(active_theme_path(dir)).ok()?;
    toml::from_str::<ActiveTheme>(&contents).ok().map(|a| a.theme)
}

fn load_theme(dir: &Path, id: &str) -> Option<Theme> {
    let contents = std::fs::read_to_string(theme_path(dir, id)).ok()?;
    let file: ThemeFile = toml::from_str(&contents).ok()?;
    Some(Theme::default().with_colors(file.colors))
}

fn bootstrap(dir: &Path) {
    if std::fs::create_dir_all(themes_dir(dir)).is_err() {
        return;
    }
    let file = ThemeFile {
        colors: ColorScheme::dark(),
    };
    if let Ok(toml) = toml::to_string_pretty(&file) {
        let _ = std::fs::write(theme_path(dir, DEFAULT_THEME_ID), toml);
    }
    let active = ActiveTheme {
        theme: DEFAULT_THEME_ID.into(),
    };
    if let Ok(toml) = toml::to_string_pretty(&active) {
        let _ = std::fs::write(active_theme_path(dir), toml);
    }
}

fn active_theme_path(dir: &Path) -> PathBuf {
    dir.join("active_theme.toml")
}

fn themes_dir(dir: &Path) -> PathBuf {
    dir.join("themes")
}

fn theme_path(dir: &Path, id: &str) -> PathBuf {
    themes_dir(dir).join(format!("{id}.toml"))
}

#[cfg(target_os = "windows")]
fn cream_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cream")
}

#[cfg(not(target_os = "windows"))]
fn cream_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cream")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstraps_then_falls_back_on_corruption() {
        let tmp = std::env::temp_dir().join(format!("creamui-theme-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &tmp);
        }

        let first = active_theme();
        assert_eq!(first.colors, ColorScheme::dark());
        assert!(active_theme_path(&tmp.join("cream")).exists());
        assert!(theme_path(&tmp.join("cream"), DEFAULT_THEME_ID).exists());

        std::fs::write(theme_path(&tmp.join("cream"), DEFAULT_THEME_ID), "not valid toml").unwrap();
        let after_corruption = active_theme();
        assert_eq!(after_corruption.colors, ColorScheme::dark());

        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
