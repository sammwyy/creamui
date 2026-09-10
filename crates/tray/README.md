# creamui-tray

Platform-tray layer for CreamUI. It contains no widget or renderer state:
native backends emit menu ids and the host runtime decides how to handle them.

## Feature and platform matrix

| Cargo feature | Target | Backend | Extra native dependency |
| --- | --- | --- | --- |
| `linux-status-notifier` | Linux | freedesktop StatusNotifierItem over D-Bus | none (pure Rust) |
| `linux-gtk` | Linux | Reserved for a future GTK/AppIndicator fallback | none today |
| `windows-native` | Windows | Reserved for a future Win32 notification-area backend | none today |
| `macos-native` | macOS | Reserved for a future NSStatusItem backend | none today |

The crate's types compile without any backend feature. `TrayBuilder::build`
returns `TrayError::UnsupportedPlatform` until a backend is selected for the
target. `creamui-render` maps its opt-in `tray` feature to
`linux-status-notifier`, so ordinary CreamUI binaries do not include D-Bus or
tray code.
