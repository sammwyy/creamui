use crate::*;
use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) fn query() -> SystemAppearance {
    #[cfg(feature = "portal")]
    if let Some(appearance) = portal() {
        return appearance;
    }
    gsettings()
}

#[cfg(feature = "portal")]
fn portal() -> Option<SystemAppearance> {
    use dbus::{
        arg::{PropMap, RefArg},
        blocking::Connection,
    };
    use std::collections::HashMap;
    let connection = Connection::new_session().ok()?;
    let proxy = connection.with_proxy(
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        Duration::from_millis(250),
    );
    let (settings,): (HashMap<String, PropMap>,) = proxy
        .method_call(
            "org.freedesktop.portal.Settings",
            "ReadAll",
            (vec!["org.freedesktop.appearance", "org.creamui.appearance"],),
        )
        .ok()?;
    let values = settings.get("org.freedesktop.appearance")?;
    let scheme = values
        .get("color-scheme")
        .and_then(|v| v.0.as_u64())
        .map(|v| SystemColorScheme::from_portal(v as u32));
    let mut appearance = SystemAppearance {
        source: AppearanceSource::Portal,
        color_scheme: scheme.unwrap_or_default(),
        ..Default::default()
    };
    if let Some(accent) = values.get("accent-color").and_then(|v| v.0.as_iter()) {
        let rgb: Vec<f64> = accent.filter_map(RefArg::as_f64).collect();
        if rgb.len() == 3 && rgb.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)) {
            appearance.accent = Some(Color::rgb(
                (rgb[0] * 255.).round() as u8,
                (rgb[1] * 255.).round() as u8,
                (rgb[2] * 255.).round() as u8,
            ));
        }
    }
    appearance.high_contrast = values.get("contrast").and_then(|v| v.0.as_u64()) == Some(1);
    appearance.reduced_motion = values.get("reduced-motion").and_then(|v| v.0.as_u64()) == Some(1);
    appearance.variant = settings
        .get("org.creamui.appearance")
        .and_then(|v| v.get("variant"))
        .and_then(|v| v.0.as_str())
        .filter(|v| !v.trim().is_empty())
        .map(str::to_owned);
    Some(appearance)
}

fn gsettings() -> SystemAppearance {
    let mode = read_setting("org.gnome.desktop.interface", "color-scheme");
    let mut appearance = SystemAppearance::default();
    if let Some(mode) = mode {
        appearance.source = AppearanceSource::GSettings;
        appearance.color_scheme = match mode.as_str() {
            "prefer-dark" => SystemColorScheme::Dark,
            "prefer-light" => SystemColorScheme::Light,
            _ => SystemColorScheme::NoPreference,
        };
    } else if let Some(theme) = read_setting("org.gnome.desktop.interface", "gtk-theme") {
        appearance.source = AppearanceSource::GSettings;
        let theme = theme.to_ascii_lowercase();
        appearance.color_scheme = if theme.ends_with("-dark") || theme.ends_with(":dark") {
            SystemColorScheme::Dark
        } else if ["adwaita", "breeze"].contains(&theme.as_str()) || theme.ends_with("-light") {
            SystemColorScheme::Light
        } else {
            SystemColorScheme::NoPreference
        };
    }
    if appearance.source != AppearanceSource::Unavailable {
        appearance.high_contrast =
            read_setting("org.gnome.desktop.a11y.interface", "high-contrast").as_deref()
                == Some("true");
        appearance.reduced_motion =
            read_setting("org.gnome.desktop.interface", "enable-animations").as_deref()
                == Some("false");
    }
    appearance
}

fn read_setting(schema: &str, key: &str) -> Option<String> {
    let mut child = Command::new("gsettings")
        .args(["get", schema, key])
        .env("GIO_USE_VFS", "local")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_millis(150);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    if !status.success() {
        return None;
    }
    let mut result = String::new();
    child
        .stdout
        .take()?
        .take(4096)
        .read_to_string(&mut result)
        .ok()?;
    Some(result.trim().trim_matches('\'').to_owned())
}
