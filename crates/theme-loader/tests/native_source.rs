use creamui_theme::{CornerStyle, ResolvedAppearance, Theme};
use creamui_theme_loader::{appearance_snapshot, SystemThemeLoader};
use std::{fs, process::Command};

#[test]
fn native_source_and_process_overrides_remain_independent() {
    if let Ok(path) = std::env::var("CREAMUI_SNAPSHOT_TEST_CHILD") {
        let loaded = SystemThemeLoader::new().load().unwrap();
        if std::env::var_os("CREAMUI_VARIANT").is_some() {
            assert_eq!(loaded.variant_id, "light");
            assert_ne!(loaded.theme.button_radius, 27.);
        } else {
            assert_eq!(loaded.theme_id, "desktop-example");
            assert_eq!(loaded.theme.button_radius, 27.);
            assert_eq!(loaded.theme.typography.body, 22.);
            let mut changed = loaded;
            changed.theme.button_radius = 9.;
            fs::write(&path, appearance_snapshot(&changed).unwrap()).unwrap();
            assert_eq!(
                SystemThemeLoader::new().load().unwrap().theme.button_radius,
                9.
            );
        }
        return;
    }
    let root = std::env::temp_dir().join(format!("cream-native-source-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("native.toml");
    let mut theme = Theme::dark();
    theme.button_radius = 27.;
    theme.typography.body = 22.;
    let appearance = ResolvedAppearance {
        theme_id: "desktop-example".into(),
        variant_id: "night".into(),
        accent: theme.colors.accent,
        font_family: Some("Sans".into()),
        corners: CornerStyle::Round,
        theme,
    };
    for override_variant in [false, true] {
        fs::write(&path, appearance_snapshot(&appearance).unwrap()).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "native_source_and_process_overrides_remain_independent",
                "--nocapture",
            ])
            .env("CREAMUI_SNAPSHOT_TEST_CHILD", &path)
            .env("CREAMUI_APPEARANCE", &path)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env_remove("CREAMUI_THEME")
            .env_remove("CREAMUI_VARIANT")
            .env_remove("CREAMUI_ACCENT")
            .env_remove("CREAMUI_FONT");
        if override_variant {
            command.env("CREAMUI_VARIANT", "light");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::remove_dir_all(root).unwrap();
}
