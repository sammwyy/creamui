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
