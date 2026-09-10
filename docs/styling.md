# Styling scope

CreamUI uses CSS-like names and values for individual style declarations, not a CSS runtime. `Style`, `StyleProp`, and JSX inline props compile into typed values when the UI is built; rendering does not parse strings.

```rust
use creamui::{Style, StyleProp};

let card = Style::new()
    .width("75%")
    .padding("12px")
    .property(StyleProp::parse("background", "primary")?);
```

`StyleProp::parse` accepts a single supported property/value pair. Lengths use `px`, `%`, or `auto` where valid (`auto` is valid for margins and positioning, not padding or gaps); colors accept `#rrggbb`, `#rrggbbaa`, and theme tokens such as `primary` or `var(--accent)`. It covers common paint and typography values plus layout declarations for size constraints, flex, grid display, gaps, spacing, alignment, and positioning.

There is intentionally no parser for stylesheets, selectors, specificity, cascade, inheritance, media queries, or CSS shorthands. For example, `padding: "8px 12px"` is not a supported declaration; use `.padding(...)` or the directional properties instead. Reusable styles are ordinary Rust values, and later builders or JSX inline props override the earlier declaration.

Use `creamui_core::layout::Style` directly only for advanced Taffy configuration that has no common `StyleProp` yet.
