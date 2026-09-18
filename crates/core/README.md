# creamui-core

The foundation of CreamUI's native widget system.

This crate defines `Widget`, `Painter`, scene construction, hit testing, and
CreamUI's common `Style` (layout + paint + typography + interaction states).
Taffy's layout-only `Style` remains re-exported under `creamui_core::layout`
and converts into the common style for incremental migration. Most
applications should use this crate together with `creamui-widgets`,
`creamui-render`, and `creamui-theme` rather than implementing the low-level
traits directly.

See the [CreamUI getting-started guide](https://github.com/sammwyy/creamui/blob/main/docs/getting-started.md) for an application-level introduction.

Paint styles support solid backgrounds, two-stop linear gradients, and outer
box shadows. Angles use CSS direction semantics.

```rust
use creamui_core::{BoxShadow, LinearGradient, Style};
use creamui_theme::Color;

let panel = Style::new()
    .background(LinearGradient::new(
        135.0,
        Color::rgb(30, 32, 40),
        Color::rgb(12, 13, 18),
    ))
    .box_shadow(BoxShadow::new(
        0.0,
        8.0,
        24.0,
        0.0,
        Color::rgba(0, 0, 0, 120),
    ));
```

The declaration parser accepts the equivalent CSS-like values:
`linear-gradient(135deg, #1e2028, #0c0d12)` and
`box-shadow: 0px 8px 24px 0px #00000078`.
