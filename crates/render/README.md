# creamui-render

Native window hosting and frame presentation for CreamUI.

`run` creates a desktop window, builds a widget tree reactively, lays it out, rasterizes it, and presents the resulting frame. GPU presentation is the default; a CPU-only `softbuffer` backend is available through `WindowOptions` when that is a better fit for the host environment.

```rust
creamui_render::run(options, theme.surface, |_| {}, build_ui);
```

For a complete example, see the [CreamUI repository](https://github.com/sammwyy/creamui).

## Background apps and system tray

`AppBuilder` retains the usual desktop behavior by default: it exits when
its last real window closes. For an application whose lifetime is owned by a
tray icon or another background service, use `keep_running()` and explicitly
call `AppHandle::exit()` when it is time to terminate.

The optional Linux `tray` feature uses the freedesktop StatusNotifierItem
protocol over D-Bus. It does not use GTK or AppIndicator. Tray menu handlers
receive an `AppHandle`, so they can update reactive signals or call
`append_window()` to open a new top-level window while the event loop is
already running. A window can use `WindowOptions { close_behavior:
CloseBehavior::Hide, .. }` and later be restored with `WindowHandle::show()`
without losing its state on platforms that support native hiding. Wayland does
not; for portable background applications, close the window and create it
again from the tray action, as the workspace example does.

The desktop environment must expose a StatusNotifierItem host. KDE Plasma and
most Linux panels do; GNOME commonly needs a StatusNotifier/AppIndicator shell
extension to display the icon, but the application itself still has no GTK
dependency.

Run the workspace's complete example with:

```sh
cargo run -p tray
```
