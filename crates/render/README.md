# creamui-render

Native window hosting and frame presentation for CreamUI.

`run` creates a desktop window, builds a widget tree reactively, lays it out and records it into a display list. Each frame is diffed against the one on screen, so only changed regions are redrawn and presented.

- **GPU** (default): an instanced `wgpu` pipeline draws calculated damage into a retained color texture (SDF rounded rects, borders, lines, glyph atlas, images). Falls back to CPU when no adapter is usable.
- **CPU**: `tiny-skia` replays only the damaged regions; Wayland presents through an `Argb8888` `wl_shm` buffer so transparent windows keep per-pixel alpha, Android copies premultiplied RGBA directly into its native buffer and honors the expanded damage returned by the platform; other platforms use `softbuffer`.

Force a backend with `CUI_OVERRIDE_RENDER_BACKEND=gpu|cpu`. `CUI_DUMP_FRAME=path.png` writes every presented frame to disk.

GPU damage regions are cleared before their primitives are replayed, preserving
correct translucent blending. Partial frames cull instance ranges outside the
damage. Every acquired surface buffer receives the complete retained texture,
by texture copy when supported or a fullscreen blit otherwise. Presentation
still transfers the full color texture; content rasterization follows damage.
Resizing allocates a new retained texture and forces a full draw.

`HeadlessGpu::render_damage_to_pixels` uses the same retained path for testing.
`GpuRenderer::render_damage` can also draw into a caller-owned target whose
pixels outside the damage already contain the preceding frame.

```rust
creamui_render::run(options, theme.surface, |_| {}, build_ui);
```

For a complete example, see the [CreamUI repository](https://github.com/sammwyy/creamui).

## Window decorations

`WindowOptions::decorations = true` requests a native frame. The native Wayland
backend asks for server-side decorations through `xdg-decoration`, but honors
the compositor's answer instead of assuming that request was accepted.
`use_window_decorations()` reads the reactive result while building content;
`WindowHandle::decorations()` exposes the same state to callbacks and effects.

The result distinguishes `Pending`, `Server`, `Client`, `Hybrid`, and explicitly
undecorated `None`. Draw fallback controls only for `Client`, never for
`Pending`. On compositors without `xdg-decoration`, a decorated window resolves
to `Client` after its first configure.

With `platform-window-integration-blair`, request
`handle.set_compositor_integration(Some(CompositorIntegrationRequest::Hybrid))`
to allow compositor-owned controls over the client surface. Unsupported
compositors keep their standard decoration result. `WindowDecorations::controls`
reports the logical-pixel rectangle to reserve in `Hybrid` mode. Ownership and
rectangle changes rebuild subscribed content, including when the compositor
withdraws hybrid mode and restores its standard frame.

Decorations do not round or clip the application's root surface. Leave its
outline to the compositor so the window, opacity and background blur share
the same shape.

### Moving and resizing without stealing content clicks

The app chooses its empty draggable regions with `CUIWindowDragArea`. This is
background metadata, not a transparent button that intercepts input. Buttons,
inputs, sliders, and intrinsic content such as text and icons inside or over
the region keep priority automatically. No application-level hit-test or
per-button exception is needed. Regions honor viewport clipping and modals.

CreamUI passes the initiating pointer-press serial to Wayland's native
`xdg_toplevel.move` and `resize` requests, including when using the window drag
handle directly. Resizable client, undecorated, and hybrid windows expose all
eight resize edges; normal server decorations leave their edges to the
compositor. A resizable window's initial size is not imposed as its minimum.

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

Android releases CPU and GPU surfaces during suspension, preserves the window's
reactive state, and recreates its presenter on resume. The first resumed frame
repaints the entire surface, including after orientation or density changes.
One activity window is supported. Additional window and native popup requests
log a warning and do not invoke their ready callbacks; use hosted overlays for
secondary views. A closed window can be replaced when the application uses
`AppBuilder::keep_running()`.
