# creamui

The recommended starting point for CreamUI applications.

By default, `creamui` re-exports the native runtime: widgets, theme tokens, reactive signals, layout primitives, and window rendering. Optional integrations are opt-in so applications only compile what they use.

```toml
[dependencies]
creamui = { version = "0.1", features = ["jsx", "image"] }
```

| Feature | Enables |
|---|---|
| `gpu` (default) | `wgpu` renderer with CPU fallback |
| `image` | `creamui-image` with PNG decoding |
| `image-jpeg` / `image-webp` | JPEG or WebP decoding, respectively |
| `jsx` | JSX macros and runtime support |
| `router` | Declarative routing, provider/outlet, reactive hooks and platform history |
| `abi` | C-compatible ABI types |
| `dynamic` | Dynamic runtime client and ABI types |
| `ffi` | Shared-library C ABI and ABI types |
| `devtools` | `creamui-devtools`, re-exported as `creamui::devtools` |
| `system-theme` | Automatic theme resolution, live `AppBuilder::system_theme()`, host queries as `creamui::appearance`, and `use_system_appearance()` |
| `theme-watch` | Host, session snapshot and config notifications as `creamui::theme_watch`; native Linux and Windows backends |
| `full` | Every optional integration |

Use modules such as `creamui::widgets`, `creamui::theme`, and `creamui::render`, or import common items directly from `creamui`.

For CPU rendering without compiling GPU support, use
`creamui = { version = "0.2", default-features = false, features = ["platform-winit"] }`.
This defaults to `RenderBackend::Cpu`; `platform-wayland` and `platform-x11`
can be selected instead. Custom builds that disable defaults must explicitly
enable `gpu` when they need GPU rendering.

For local development, enable the optional devtools integration and install it
before opening any windows. F3 then shows or hides the per-window performance
overlay:

```toml
creamui = { version = "0.1", features = ["devtools"] }
```

```rust
fn main() {
    creamui::devtools::init();
    // creamui::run(...)
}
```

See the [CreamUI getting-started guide](https://github.com/sammwyy/creamui/blob/main/docs/getting-started.md).
