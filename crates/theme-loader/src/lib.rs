use creamui_theme::{
    AccentPreset, AppearanceSelection, Color, ColorScheme, ResolvedAppearance, Theme,
    ThemeDefinition,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug)]
pub enum ThemeLoadError {
    Io(std::io::Error),
    ConfigurationPathUnavailable,
    InvalidAppearance(toml::de::Error),
    InvalidTheme {
        path: PathBuf,
        source: toml::de::Error,
    },
    InvalidColor {
        value: String,
    },
    InvalidThemeId(String),
    ThemeNotFound(String),
    VariantNotFound {
        theme: String,
        variant: String,
    },
    InvalidThemeDefinition(String),
}

impl fmt::Display for ThemeLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "theme I/O failed: {error}"),
            Self::ConfigurationPathUnavailable => {
                f.write_str("system appearance configuration path is unavailable")
            }
            Self::InvalidAppearance(error) => {
                write!(f, "invalid appearance configuration: {error}")
            }
            Self::InvalidTheme { path, source } => {
                write!(f, "invalid theme file {}: {source}", path.display())
            }
            Self::InvalidColor { value } => write!(f, "invalid color {value}"),
            Self::InvalidThemeId(id) => write!(f, "invalid theme id {id}"),
            Self::ThemeNotFound(id) => write!(f, "theme {id} was not found"),
            Self::VariantNotFound { theme, variant } => {
                write!(f, "theme {theme} has no {variant} variant")
            }
            Self::InvalidThemeDefinition(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ThemeLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidAppearance(error) => Some(error),
            Self::InvalidTheme { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct SystemThemeLoader {
    selection: AppearanceSelection,
}

impl SystemThemeLoader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn theme(mut self, id: impl Into<String>) -> Self {
        self.selection.theme = Some(id.into());
        self
    }

    pub fn variant(mut self, id: impl Into<String>) -> Self {
        self.selection.variant = Some(id.into());
        self
    }

    pub fn accent(mut self, color: Color) -> Self {
        self.selection.accent = Some(color);
        self
    }

    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.selection.font_family = Some(family.into());
        self
    }

    pub fn load(self) -> Result<ResolvedAppearance, ThemeLoadError> {
        self.load_from_paths(config_path(), data_dirs())
    }

    pub fn load_from_paths(
        self,
        appearance_path: Option<PathBuf>,
        data_dirs: Vec<PathBuf>,
    ) -> Result<ResolvedAppearance, ThemeLoadError> {
        let config = match appearance_path.as_deref() {
            Some(path) if path.is_file() => read_appearance(path)?,
            _ => AppearanceSelection::default(),
        };
        let environment = environment_selection()?;
        let selection = merge_selection(config, environment, self.selection);
        let theme_id = selection.theme.clone().unwrap_or_else(|| "default".into());
        let theme = find_theme(&theme_id, &data_dirs)?;
        let variant_id = selection
            .variant
            .clone()
            .unwrap_or_else(|| theme.default_variant.clone());
        let mut resolved =
            theme
                .variant(&variant_id)
                .ok_or_else(|| ThemeLoadError::VariantNotFound {
                    theme: theme_id.clone(),
                    variant: variant_id.clone(),
                })?;
        let accent = selection
            .accent
            .or(theme.default_accent)
            .unwrap_or(resolved.colors.accent);
        resolved = resolved.with_accent(accent);
        Ok(ResolvedAppearance {
            theme_id,
            variant_id,
            accent,
            theme: resolved,
            font_family: selection.font_family,
        })
    }
}

pub fn builtin_theme() -> ThemeDefinition {
    ThemeDefinition::builtin_default()
}

pub fn list_themes() -> Result<Vec<ThemeDefinition>, ThemeLoadError> {
    let mut themes = BTreeMap::new();
    for root in data_dirs() {
        let directory = root.join("themes");
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(id) = entry.file_name().into_string() else {
                continue;
            };
            if themes.contains_key(&id) {
                continue;
            }
            if let Ok(mut theme) = find_theme(&id, std::slice::from_ref(&root)) {
                theme.id = id.clone();
                themes.insert(id, theme);
            }
        }
    }
    themes
        .entry("default".to_owned())
        .or_insert_with(ThemeDefinition::builtin_default);
    Ok(themes.into_values().collect())
}

pub fn write_system_appearance(selection: &AppearanceSelection) -> Result<(), ThemeLoadError> {
    let path = config_path().ok_or(ThemeLoadError::ConfigurationPathUnavailable)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ThemeLoadError::Io)?;
    }
    let mut contents = String::new();
    if let Some(theme) = &selection.theme {
        contents.push_str(&format!("theme = {}\n", toml::Value::String(theme.clone())));
    }
    if let Some(variant) = &selection.variant {
        contents.push_str(&format!(
            "variant = {}\n",
            toml::Value::String(variant.clone())
        ));
    }
    if let Some(accent) = selection.accent {
        contents.push_str(&format!(
            "accent = \"#{:02x}{:02x}{:02x}{:02x}\"\n",
            accent.r, accent.g, accent.b, accent.a
        ));
    }
    if let Some(font_family) = &selection.font_family {
        contents.push_str(&format!(
            "font_family = {}\n",
            toml::Value::String(font_family.clone())
        ));
    }
    fs::write(path, contents).map_err(ThemeLoadError::Io)
}

pub fn config_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("CreamUI").join("appearance.toml"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|path| path.join("cream").join("appearance.toml"))
    }
}

pub fn data_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .into_iter()
            .collect()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut paths = env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
            .into_iter()
            .collect::<Vec<_>>();
        let system =
            env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
        paths.extend(env::split_paths(&system));
        paths
    }
}

pub fn find_theme(id: &str, data_dirs: &[PathBuf]) -> Result<ThemeDefinition, ThemeLoadError> {
    if id == "default" {
        for root in data_dirs {
            let path = theme_path(root, id)?;
            if path.is_file() {
                return read_theme(&path);
            }
        }
        return Ok(ThemeDefinition::builtin_default());
    }
    for root in data_dirs {
        let path = theme_path(root, id)?;
        if path.is_file() {
            return read_theme(&path);
        }
    }
    Err(ThemeLoadError::ThemeNotFound(id.into()))
}

fn theme_path(root: &Path, id: &str) -> Result<PathBuf, ThemeLoadError> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(ThemeLoadError::InvalidThemeId(id.into()));
    }
    Ok(root
        .join("themes")
        .join(id)
        .join("cream")
        .join("theme.toml"))
}

fn read_appearance(path: &Path) -> Result<AppearanceSelection, ThemeLoadError> {
    let text = fs::read_to_string(path).map_err(ThemeLoadError::Io)?;
    let file: AppearanceFile = toml::from_str(&text).map_err(ThemeLoadError::InvalidAppearance)?;
    selection_from_file(file)
}

fn environment_selection() -> Result<AppearanceSelection, ThemeLoadError> {
    let accent = match env::var("CREAMUI_ACCENT") {
        Ok(value) => Some(parse_color(value)?),
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => None,
    };
    Ok(AppearanceSelection {
        theme: env::var("CREAMUI_THEME").ok(),
        variant: env::var("CREAMUI_VARIANT").ok(),
        accent,
        font_family: env::var("CREAMUI_FONT").ok(),
    })
}

fn merge_selection(
    base: AppearanceSelection,
    middle: AppearanceSelection,
    top: AppearanceSelection,
) -> AppearanceSelection {
    AppearanceSelection {
        theme: top.theme.or(middle.theme).or(base.theme),
        variant: top.variant.or(middle.variant).or(base.variant),
        accent: top.accent.or(middle.accent).or(base.accent),
        font_family: top.font_family.or(middle.font_family).or(base.font_family),
    }
}

fn read_theme(path: &Path) -> Result<ThemeDefinition, ThemeLoadError> {
    let text = fs::read_to_string(path).map_err(ThemeLoadError::Io)?;
    let file: ThemeFile = toml::from_str(&text).map_err(|source| ThemeLoadError::InvalidTheme {
        path: path.into(),
        source,
    })?;
    theme_from_file(file)
}

#[derive(Deserialize)]
struct AppearanceFile {
    theme: Option<String>,
    variant: Option<String>,
    accent: Option<String>,
    font_family: Option<String>,
}

#[derive(Deserialize)]
struct ThemeFile {
    id: String,
    name: String,
    default_variant: String,
    default_accent: Option<String>,
    #[serde(default)]
    accent_presets: Vec<AccentPresetFile>,
    variants: BTreeMap<String, VariantFile>,
}

#[derive(Deserialize)]
struct AccentPresetFile {
    id: String,
    name: String,
    color: String,
}

#[derive(Deserialize)]
struct VariantFile {
    colors: ColorSchemeFile,
}

#[derive(Deserialize)]
struct ColorSchemeFile {
    surface: String,
    surface_elevated: String,
    surface_hover: String,
    accent: String,
    accent_hover: String,
    accent_pressed: String,
    selection_background: String,
    selection_text: String,
    text_primary: String,
    text_secondary: String,
    text_disabled: String,
    border: String,
    border_strong: String,
    danger: String,
    warning: String,
    success: String,
}

fn selection_from_file(file: AppearanceFile) -> Result<AppearanceSelection, ThemeLoadError> {
    Ok(AppearanceSelection {
        theme: file.theme,
        variant: file.variant,
        accent: file.accent.map(parse_color).transpose()?,
        font_family: file.font_family,
    })
}

fn parse_color(value: String) -> Result<Color, ThemeLoadError> {
    Color::from_str(&value).map_err(|_| ThemeLoadError::InvalidColor { value })
}

fn theme_from_file(file: ThemeFile) -> Result<ThemeDefinition, ThemeLoadError> {
    if file.id.is_empty()
        || file.name.is_empty()
        || !file.variants.contains_key(&file.default_variant)
    {
        return Err(ThemeLoadError::InvalidThemeDefinition(
            "a theme needs an id, name, and existing default variant".into(),
        ));
    }
    let variants = file
        .variants
        .into_iter()
        .map(|(id, variant)| {
            Ok((
                id,
                Theme::default().with_colors(colors_from_file(variant.colors)?),
            ))
        })
        .collect::<Result<_, ThemeLoadError>>()?;
    let accent_presets = file
        .accent_presets
        .into_iter()
        .map(|preset| {
            Ok(AccentPreset {
                id: preset.id,
                name: preset.name,
                color: parse_color(preset.color)?,
            })
        })
        .collect::<Result<_, ThemeLoadError>>()?;
    Ok(ThemeDefinition {
        id: file.id,
        name: file.name,
        default_variant: file.default_variant,
        variants,
        accent_presets,
        default_accent: file.default_accent.map(parse_color).transpose()?,
    })
}

fn colors_from_file(file: ColorSchemeFile) -> Result<ColorScheme, ThemeLoadError> {
    Ok(ColorScheme {
        surface: parse_color(file.surface)?,
        surface_elevated: parse_color(file.surface_elevated)?,
        surface_hover: parse_color(file.surface_hover)?,
        accent: parse_color(file.accent)?,
        accent_hover: parse_color(file.accent_hover)?,
        accent_pressed: parse_color(file.accent_pressed)?,
        selection_background: parse_color(file.selection_background)?,
        selection_text: parse_color(file.selection_text)?,
        text_primary: parse_color(file.text_primary)?,
        text_secondary: parse_color(file.text_secondary)?,
        text_disabled: parse_color(file.text_disabled)?,
        border: parse_color(file.border)?,
        border_strong: parse_color(file.border_strong)?,
        danger: parse_color(file.danger)?,
        warning: parse_color(file.warning)?,
        success: parse_color(file.success)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn temporary_dir(name: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "creamui-theme-loader-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_theme(root: &Path, id: &str) {
        let path = root.join("themes").join(id).join("cream");
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("theme.toml"),
            r##"
id = "custom"
name = "Custom"
default_variant = "latte"
default_accent = "#010203"

[[accent_presets]]
id = "blue"
name = "Blue"
color = "#0000ff"

[variants.latte.colors]
surface = "#111111"
surface_elevated = "#121212"
surface_hover = "#131313"
accent = "#141414"
accent_hover = "#151515"
accent_pressed = "#161616"
selection_background = "#171717"
selection_text = "#181818"
text_primary = "#191919"
text_secondary = "#202020"
text_disabled = "#212121"
border = "#222222"
border_strong = "#232323"
danger = "#242424"
warning = "#252525"
success = "#262626"

[variants.oled.colors]
surface = "#000000"
surface_elevated = "#010101"
surface_hover = "#020202"
accent = "#030303"
accent_hover = "#040404"
accent_pressed = "#050505"
selection_background = "#060606"
selection_text = "#070707"
text_primary = "#080808"
text_secondary = "#090909"
text_disabled = "#101010"
border = "#111111"
border_strong = "#121212"
danger = "#131313"
warning = "#141414"
success = "#151515"
"##,
        )
        .unwrap();
    }

    #[test]
    fn missing_appearance_uses_the_builtin_default() {
        let _guard = lock();
        let appearance = temporary_dir("missing").join("appearance.toml");
        let resolved = SystemThemeLoader::new()
            .load_from_paths(Some(appearance), vec![])
            .unwrap();
        assert_eq!(resolved.theme_id, "default");
        assert_eq!(resolved.variant_id, "dark");
    }

    #[test]
    fn resolves_arbitrary_variants_and_custom_accents() {
        let _guard = lock();
        let root = temporary_dir("variants");
        write_theme(&root, "custom");
        let accent = Color::rgb(255, 117, 181);
        let resolved = SystemThemeLoader::new()
            .theme("custom")
            .variant("oled")
            .accent(accent)
            .load_from_paths(None, vec![root.clone()])
            .unwrap();
        assert_eq!(resolved.variant_id, "oled");
        assert_eq!(resolved.accent, accent);
        assert_eq!(resolved.theme.colors.accent, accent);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn font_family_is_absent_by_default_and_honors_the_builder() {
        let _guard = lock();
        let appearance = temporary_dir("no-font").join("appearance.toml");
        let resolved = SystemThemeLoader::new()
            .load_from_paths(Some(appearance), vec![])
            .unwrap();
        assert_eq!(resolved.font_family, None);

        let appearance = temporary_dir("with-font").join("appearance.toml");
        let resolved = SystemThemeLoader::new()
            .font_family("Inter")
            .load_from_paths(Some(appearance), vec![])
            .unwrap();
        assert_eq!(resolved.font_family, Some("Inter".to_owned()));
    }

    #[test]
    fn font_family_is_read_from_the_appearance_file() {
        let _guard = lock();
        let root = temporary_dir("font-file");
        let appearance = root.join("appearance.toml");
        fs::write(&appearance, "font_family = 'Fira Code'\n").unwrap();
        let resolved = SystemThemeLoader::new()
            .load_from_paths(Some(appearance), vec![])
            .unwrap();
        assert_eq!(resolved.font_family, Some("Fira Code".to_owned()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn writing_the_system_appearance_persists_the_font_family() {
        let _guard = lock();
        let root = temporary_dir("write-font");
        let previous_config = env::var_os("XDG_CONFIG_HOME");
        unsafe { env::set_var("XDG_CONFIG_HOME", &root) };
        write_system_appearance(&AppearanceSelection {
            font_family: Some("Fira Code".to_owned()),
            ..Default::default()
        })
        .unwrap();
        let contents = fs::read_to_string(root.join("cream/appearance.toml")).unwrap();
        assert!(contents.contains("font_family = \"Fira Code\""));
        unsafe { restore("XDG_CONFIG_HOME", previous_config) };
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_a_missing_variant() {
        let _guard = lock();
        let root = temporary_dir("missing-variant");
        write_theme(&root, "custom");
        let error = SystemThemeLoader::new()
            .theme("custom")
            .variant("missing")
            .load_from_paths(None, vec![root.clone()])
            .unwrap_err();
        assert!(matches!(error, ThemeLoadError::VariantNotFound { .. }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn config_and_programmatic_selection_have_expected_precedence() {
        let _guard = lock();
        let root = temporary_dir("precedence");
        write_theme(&root, "custom");
        let appearance = root.join("appearance.toml");
        fs::write(
            &appearance,
            "theme = 'custom'\nvariant = 'latte'\naccent = '#abcdef'\n",
        )
        .unwrap();
        let resolved = SystemThemeLoader::new()
            .variant("oled")
            .accent(Color::rgb(1, 2, 3))
            .load_from_paths(Some(appearance), vec![root.clone()])
            .unwrap();
        assert_eq!(resolved.variant_id, "oled");
        assert_eq!(resolved.accent, Color::rgb(1, 2, 3));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn themes_follow_data_directory_order() {
        let _guard = lock();
        let first = temporary_dir("first");
        let second = temporary_dir("second");
        write_theme(&first, "custom");
        write_theme(&second, "custom");
        let first_path = first.join("themes/custom/cream/theme.toml");
        let contents = fs::read_to_string(&first_path)
            .unwrap()
            .replace("name = \"Custom\"", "name = \"First\"");
        fs::write(first_path, contents).unwrap();
        let found = find_theme("custom", &[first.clone(), second.clone()]).unwrap();
        assert_eq!(found.name, "First");
        assert!(matches!(
            find_theme("missing", std::slice::from_ref(&first)),
            Err(ThemeLoadError::ThemeNotFound(_))
        ));
        fs::remove_dir_all(first).unwrap();
        fs::remove_dir_all(second).unwrap();
    }

    #[test]
    fn xdg_paths_use_environment_overrides() {
        let _guard = lock();
        let root = temporary_dir("xdg");
        let previous_config = env::var_os("XDG_CONFIG_HOME");
        let previous_data_home = env::var_os("XDG_DATA_HOME");
        let previous_data_dirs = env::var_os("XDG_DATA_DIRS");
        unsafe {
            env::set_var("XDG_CONFIG_HOME", root.join("config"));
            env::set_var("XDG_DATA_HOME", root.join("data"));
            env::set_var(
                "XDG_DATA_DIRS",
                env::join_paths([root.join("one"), root.join("two")]).unwrap(),
            );
        }
        assert_eq!(
            config_path(),
            Some(root.join("config/cream/appearance.toml"))
        );
        assert_eq!(
            data_dirs(),
            vec![root.join("data"), root.join("one"), root.join("two")]
        );
        unsafe { restore("XDG_CONFIG_HOME", previous_config) };
        unsafe { restore("XDG_DATA_HOME", previous_data_home) };
        unsafe { restore("XDG_DATA_DIRS", previous_data_dirs) };
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn environment_overrides_configuration() {
        let _guard = lock();
        let root = temporary_dir("environment");
        write_theme(&root, "custom");
        let appearance = root.join("appearance.toml");
        fs::write(
            &appearance,
            "theme = 'custom'\nvariant = 'latte'\naccent = '#abcdef'\n",
        )
        .unwrap();
        let old_theme = env::var_os("CREAMUI_THEME");
        let old_variant = env::var_os("CREAMUI_VARIANT");
        let old_accent = env::var_os("CREAMUI_ACCENT");
        unsafe {
            env::set_var("CREAMUI_THEME", "custom");
            env::set_var("CREAMUI_VARIANT", "oled");
            env::set_var("CREAMUI_ACCENT", "#010203");
        }
        let resolved = SystemThemeLoader::new()
            .load_from_paths(Some(appearance), vec![root.clone()])
            .unwrap();
        assert_eq!(resolved.variant_id, "oled");
        assert_eq!(resolved.accent, Color::rgb(1, 2, 3));
        unsafe { restore("CREAMUI_THEME", old_theme) };
        unsafe { restore("CREAMUI_VARIANT", old_variant) };
        unsafe { restore("CREAMUI_ACCENT", old_accent) };
        fs::remove_dir_all(root).unwrap();
    }

    unsafe fn restore(name: &str, value: Option<std::ffi::OsString>) {
        if let Some(value) = value {
            env::set_var(name, value);
        } else {
            env::remove_var(name);
        }
    }
}
