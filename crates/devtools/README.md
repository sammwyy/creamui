# creamui-devtools

Development-only tooling for CreamUI applications. Call `init()` before
opening windows; press F3 in a CreamUI window to show or hide its overlay.

Runtime-backed windows also provide these inspection controls:

| Key | Action |
| --- | --- |
| F4 | Toggle the retained tree inspector |
| F5 | Toggle content damage regions in red |
| F6 | Toggle layout rectangles in blue |
| F7 | Toggle interactive hit regions in green |
| F8 / Shift+F8 | Select the next / previous tree node |

The tree shows node identity, parent, geometry, opacity, focusability, and
the last invalidation flags. Selection follows node identity across tree
changes. Damage comes from content display-list diffs and excludes the
debug overlays; the last changed regions remain visible during idle refreshes.
Inspection collects runtime snapshots only while an inspection control is
enabled. Tree rows are limited to the panel's visible height.

Use it either directly (typically as a development dependency):

```toml
[dev-dependencies]
creamui-devtools = "0.1"
```

or enable `creamui`'s `devtools` feature and call `creamui::devtools::init()`.

```rust
fn main() {
    creamui_devtools::init();
    // creamui::run(...)
}
```
