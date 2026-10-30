#![cfg(target_os = "linux")]
use creamui_appearance::{query, AppearanceSource, SystemColorScheme};
use std::{fs, process::Command};

#[test]
fn native_gsettings_fallback_is_independent_of_app_overrides() {
    if let Ok(expected) = std::env::var("CREAMUI_HOST_TEST_CHILD") {
        let host = query();
        assert_eq!(host.source, AppearanceSource::GSettings);
        assert_eq!(host.variant, None);
        assert_eq!(
            host.color_scheme,
            if expected == "light" {
                SystemColorScheme::Light
            } else {
                SystemColorScheme::Dark
            }
        );
        return;
    }
    let root = std::env::temp_dir().join(format!("cream-host-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let isolated = |program: &std::ffi::OsStr| {
        let mut command = Command::new(program);
        command
            .env("XDG_CONFIG_HOME", &root)
            .env("GSETTINGS_BACKEND", "keyfile")
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={}/absent-bus", root.display()),
            )
            .env_remove("CREAMUI_APPEARANCE")
            .env_remove("GSETTINGS_SCHEMA_DIR");
        command
    };
    for mode in ["light", "dark"] {
        let set = isolated(std::ffi::OsStr::new("gsettings"))
            .args([
                "set",
                "org.gnome.desktop.interface",
                "color-scheme",
                &format!("prefer-{mode}"),
            ])
            .output();
        let Ok(set) = set else {
            eprintln!("skipping: gsettings is not installed");
            break;
        };
        if !set.status.success() {
            eprintln!("skipping: color-scheme schema is not installed");
            break;
        }
        let child = isolated(std::env::current_exe().unwrap().as_os_str())
            .args([
                "--exact",
                "native_gsettings_fallback_is_independent_of_app_overrides",
            ])
            .env("CREAMUI_HOST_TEST_CHILD", mode)
            .env("CREAMUI_THEME", "app-override")
            .env("CREAMUI_VARIANT", "midnight")
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
    }
    fs::remove_dir_all(root).unwrap();
}
