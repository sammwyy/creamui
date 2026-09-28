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

There is intentionally no parser for stylesheets, selectors, specificity, cascade, inheritance, or media queries. Reusable styles are ordinary Rust values, and later builders or JSX inline props override the earlier declaration.

Use `creamui_core::layout::Style` directly only for advanced Taffy configuration that has no common `StyleProp` yet.

`aspect-ratio` accepts a positive number, a ratio such as `"16 / 9"`, or `"auto"`.
It supplies the missing dimension when width or height is automatic; explicit
width and height take precedence. Native styles and widgets support
`.aspect_ratio(16.0 / 9.0)` (or `None` to clear it), and JSX accepts
`<RawView width={320.0} aspect_ratio={16.0 / 9.0} />`.

`box-sizing` accepts `"border-box"` (the default) and `"content-box"`.
Use `.box_sizing(creamui::core::layout::BoxSizing::ContentBox)` or the
`box_sizing` JSX prop. Content-box sizes exclude padding and borders;
border-box sizes include them. Common `.border(...)` declarations reserve
space on all four sides and paint inside the border box. Interaction-state
borders reserve their maximum declared width so focus and hover cannot move
the layout. Outlines and shadows remain outside the box without affecting
layout.

`RawText`, `Text`, `Heading`, `RawPre`, `Pre`, `RawLink`, and `Link` paint text
within the resolved content box, including percentage padding and borders.

`padding`, `margin`, and `inset` accept one to four whitespace-separated values
in CSS order: all sides; vertical/horizontal; top/horizontal/bottom; or
top/right/bottom/left. Unitless `0` is accepted. Padding requires non-negative
lengths; margins and insets also accept negative lengths and `auto`.

```rust
let panel = Style::new().padding("8px 16px").margin("0 auto");
let overlay = Style::new()
    .position(creamui::core::layout::Position::Absolute)
    .inset("12px 24px 0");
```

JSX uses the same values: `<RawView padding={"8px 16px"} margin={"0 auto"} />`.
`Block`, `Flex`, and `Grid` accept those shorthands too.
`EdgeValues::new(top, right, bottom, left)` provides a typed alternative.
Directional JSX props override shorthand props, and inline props override
the `style` prop. Existing `StyleProp::Padding(LengthValue)` and
`StyleProp::Margin(LengthValue)` declarations remain supported.

Circular, two-stop radial backgrounds use `radial-gradient(circle, start, end)`
or `radial-gradient(circle at 25% 75%, start, end)`. The default center is
`50% 50%`; the radius reaches the farthest corner. Centers outside the box
are allowed. Colors support the same hexadecimal values and theme tokens as
other backgrounds. Ellipses, explicit radii, additional stops, and repeating
gradients are not supported.

```rust
use creamui::{ColorToken, RadialGradient, Style};

let spotlight = Style::new().background(
    RadialGradient::new(ColorToken::Accent, ColorToken::Surface).at(0.25, 0.75),
);
```

```rust,ignore
jsx! {
    <RawView width={240.0} aspect_ratio={2.0} padding={"12px 20px"}
        corner_radius={16.0}
        background={"radial-gradient(circle at 25% 25%, #587cff, #121830)"} />
}
```

Radial backgrounds also work in interaction state styles. CPU and GPU renderers
support rounded corners, clipping, scaling, scrolling, and premultiplied-alpha
interpolation. Each gradient records a single quad; translucent CPU rendering
only visits the affected pixels. Custom `Painter` implementations can override
`fill_radial_gradient`; its default fallback paints the average color, as for
linear gradients.

The [JSX styling gallery](../examples/jsx-styles/README.md) demonstrates these
properties together. The `universal-styles` example shows the native builder
API.
