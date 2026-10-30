# creamui-theme

Design tokens and theme state for CreamUI.

Desktop environments can supply an already-resolved native appearance through
`CREAMUI_APPEARANCE=/absolute/path/to/appearance.toml`. The generic
`creamui-theme-loader` exports `appearance_snapshot()` and
`load_appearance_snapshot()` to write/read this format, including colors,
geometry, typography and effective accessibility. The source is read again on
load; applications can use `creamui-theme-watch::ThemeListener` for live reloads.
This does not make CreamUI depend on a particular desktop's theme packages.

`Theme::default()` provides the rounded default dark appearance. `Theme::light()` keeps the same geometry and typography with a light palette. Applications can copy a theme and adjust individual color, spacing, type, and radius tokens.

```rust
use creamui_theme::{Color, Theme};

let mut theme = Theme::light();
theme.colors.accent = Color::rgb(72, 117, 255);
```

Part of [CreamUI](https://github.com/sammwyy/creamui), licensed under Apache-2.0.
