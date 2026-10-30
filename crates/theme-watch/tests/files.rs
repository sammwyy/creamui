#![cfg(any(target_os = "linux", target_os = "windows"))]
use creamui_theme_watch::ThemeListener;
use std::{fs, time::Duration};

#[test]
fn session_snapshot_changes_wake_the_listener_without_a_custom_signal() {
    let root = std::env::temp_dir().join(format!("cream-watch-files-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("appearance.toml");
    let previous = std::env::var_os("CREAMUI_APPEARANCE");
    fs::write(
        &path,
        "schema = 1\nvariant_id = 'light'\naccent = '#123456'\n",
    )
    .unwrap();
    // This integration-test process has one test; initialize before starting
    // the listener and restore only after its worker has joined.
    unsafe { std::env::set_var("CREAMUI_APPEARANCE", &path) };
    let listener = ThemeListener::start();
    assert!(listener.recv_timeout(Duration::from_secs(3)));
    fs::write(
        &path,
        "schema = 1\nvariant_id = 'midnight'\naccent = '#abcdef'\n",
    )
    .unwrap();
    assert!(
        listener.recv_timeout(Duration::from_millis(1500)),
        "snapshot edit was not observed"
    );
    drop(listener);
    unsafe {
        match previous {
            Some(value) => std::env::set_var("CREAMUI_APPEARANCE", value),
            None => std::env::remove_var("CREAMUI_APPEARANCE"),
        }
    }
    fs::remove_dir_all(root).unwrap();
}
