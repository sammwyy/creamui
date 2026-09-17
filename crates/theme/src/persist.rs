//! Loads the color theme shared by every CreamUI app from
//! `<config dir>/cream/`, so switching the active theme once applies to all
//! of them. `active_theme.toml` names the active theme id as plain text
//! (not a filesystem symlink) so this works unprivileged on Windows too.
//! Each theme lives in its own `cream/themes/<id>/` folder (`theme.toml`
//! plus an optional `thumbnail.<ext>`) so a theme can ship a preview image
//! alongside its colors.

use crate::{ColorScheme, Theme};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const DEFAULT_THEME_ID: &str = "default";
const THUMBNAIL_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];

#[derive(Serialize, Deserialize)]
struct ThemeFile {
    colors: ColorScheme,
}

#[derive(Serialize, Deserialize)]
struct ActiveTheme {
    theme: String,
}

/// One theme found under `cream/themes/`.
pub struct ThemeInfo {
    pub id: String,
    pub colors: ColorScheme,
    pub thumbnail: Option<PathBuf>,
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

/// The active theme's id, bootstrapping `cream/` the same way [`active_theme`] does.
pub fn active_theme_id() -> String {
    let dir = cream_dir();
    read_active_id(&dir).unwrap_or_else(|| {
        bootstrap(&dir);
        DEFAULT_THEME_ID.into()
    })
}

/// Every theme found under `cream/themes/`, sorted by id. Bootstraps
/// `cream/` first if it doesn't exist yet, so this always returns at least
/// the built-in default.
pub fn list_themes() -> Vec<ThemeInfo> {
    let dir = cream_dir();
    if read_active_id(&dir).is_none() {
        bootstrap(&dir);
    }
    let mut themes: Vec<ThemeInfo> = std::fs::read_dir(themes_dir(&dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let id = entry.file_name().to_str()?.to_owned();
            let contents = std::fs::read_to_string(theme_file_path(&dir, &id)).ok()?;
            let file: ThemeFile = toml::from_str(&contents).ok()?;
            Some(ThemeInfo {
                thumbnail: find_thumbnail(&theme_dir(&dir, &id)),
                id,
                colors: file.colors,
            })
        })
        .collect();
    themes.sort_by(|a, b| a.id.cmp(&b.id));
    themes
}

/// Points `active_theme.toml` at `id`. No-op returning `false` if that
/// theme doesn't exist, so the pointer never dangles.
pub fn set_active_theme(id: &str) -> bool {
    let dir = cream_dir();
    if !theme_file_path(&dir, id).is_file() {
        return false;
    }
    let active = ActiveTheme { theme: id.into() };
    let Ok(toml) = toml::to_string_pretty(&active) else {
        return false;
    };
    std::fs::write(active_theme_path(&dir), toml).is_ok()
}

fn read_active_id(dir: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(active_theme_path(dir)).ok()?;
    toml::from_str::<ActiveTheme>(&contents)
        .ok()
        .map(|a| a.theme)
}

fn load_theme(dir: &Path, id: &str) -> Option<Theme> {
    let contents = std::fs::read_to_string(theme_file_path(dir, id)).ok()?;
    let file: ThemeFile = toml::from_str(&contents).ok()?;
    Some(Theme::default().with_colors(file.colors))
}

fn find_thumbnail(dir: &Path) -> Option<PathBuf> {
    THUMBNAIL_EXTENSIONS
        .iter()
        .map(|ext| dir.join(format!("thumbnail.{ext}")))
        .find(|path| path.is_file())
}

fn bootstrap(dir: &Path) {
    if std::fs::create_dir_all(theme_dir(dir, DEFAULT_THEME_ID)).is_err() {
        return;
    }
    let file = ThemeFile {
        colors: ColorScheme::dark(),
    };
    if let Ok(toml) = toml::to_string_pretty(&file) {
        let _ = std::fs::write(theme_file_path(dir, DEFAULT_THEME_ID), toml);
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

fn theme_dir(dir: &Path, id: &str) -> PathBuf {
    themes_dir(dir).join(id)
}

fn theme_file_path(dir: &Path, id: &str) -> PathBuf {
    theme_dir(dir, id).join("theme.toml")
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
    fn bootstraps_lists_and_switches_themes() {
        let tmp = std::env::temp_dir().join(format!("creamui-theme-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        #[cfg(target_os = "windows")]
        let config_home = "APPDATA";
        #[cfg(not(target_os = "windows"))]
        let config_home = "XDG_CONFIG_HOME";
        let previous_config_home = std::env::var_os(config_home);
        unsafe {
            std::env::set_var(config_home, &tmp);
        }
        let dir = tmp.join("cream");

        let first = active_theme();
        assert_eq!(first.colors, ColorScheme::dark());
        assert!(active_theme_path(&dir).exists());
        assert!(theme_file_path(&dir, DEFAULT_THEME_ID).exists());

        std::fs::write(theme_file_path(&dir, DEFAULT_THEME_ID), "not valid toml").unwrap();
        assert_eq!(active_theme().colors, ColorScheme::dark());

        std::fs::create_dir_all(theme_dir(&dir, "light")).unwrap();
        std::fs::write(
            theme_file_path(&dir, "light"),
            toml::to_string_pretty(&ThemeFile {
                colors: ColorScheme::light(),
            })
            .unwrap(),
        )
        .unwrap();
        std::fs::write(theme_dir(&dir, "light").join("thumbnail.png"), b"fake").unwrap();

        let themes = list_themes();
        let light = themes.iter().find(|t| t.id == "light").unwrap();
        assert!(light.thumbnail.is_some());

        assert!(set_active_theme("light"));
        assert_eq!(active_theme_id(), "light");
        assert_eq!(active_theme().colors, ColorScheme::light());
        assert!(!set_active_theme("does-not-exist"));
        assert_eq!(active_theme_id(), "light");

        unsafe {
            if let Some(value) = previous_config_home {
                std::env::set_var(config_home, value);
            } else {
                std::env::remove_var(config_home);
            }
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
