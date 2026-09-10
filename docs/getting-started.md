# Getting started

CreamUI applications are normal Rust binaries. Build a widget tree in a closure, return it from `run`, and let signals rebuild that tree when state changes.

## Install

For a standard native app, add the facade crate:

```toml
[dependencies]
creamui = { version = "0.1", features = ["jsx"] }
```

Enable image support from the facade when your app displays raster assets:

```toml
creamui = { version = "0.1", features = ["jsx", "image-jpeg", "image-webp"] }
```

PNG is enabled by default. JPEG and WebP stay opt-in to keep application builds focused on the formats they use.

## Build a screen

`creamui::run` creates a window. Its final closure receives the current viewport and returns the root widget for that frame.

```rust
run(options, Theme::default().surface, |_| {}, move |viewport| {
    let theme = Theme::default();
    Box::new(jsx! {
        <Block style={my_layout(viewport)}>
            <Text>"Welcome"</Text>
        </Block>
    })
});
```

Use `Signal<T>` for UI state. Calling `get()` while building subscribes the screen to that value; changing it rebuilds the affected window on the next frame.

```rust
let enabled = Signal::new(false);
let set_enabled = enabled.clone();

jsx! {
    <Checkbox theme={&theme} checked={enabled.get()}
        on_click={move || set_enabled.update(|value| *value = !*value)} />
}
```

## Choose a layout

`creamui::widgets::layout` has a semantic flex API for layout containers:

```rust
use creamui::widgets::layout::{Align, Flex, Justify, Wrap};

let toolbar = Flex::row()
    .gap(12.0)
    .align(Align::Center)
    .justify(Justify::Between);

let cards = Flex::row()
    .gap_x(16.0)
    .gap_y(12.0)
    .wrap(Wrap::Wrap);
```

For two-dimensional layout, use `Grid` and position only the items that need
an explicit cell or span:

```rust
use creamui::widgets::layout::{Grid, GridItem, Track};

let dashboard = Grid::new()
    .template_columns([Track::px(240.0), Track::fr(1.0), Track::fr(1.0)])
    .gap(16.0)
    .child(Box::new(GridItem::new().at(1, 1).column_span(2)));
```

The same container is available in JSX when the `jsx` feature is enabled:

```rust
use creamui::core::layout::FlexDirection;

jsx! {
    <Flex direction={FlexDirection::Column} gap={12.0}
        align={Align::Center} justify={Justify::Center}>
        <Text>"Centered content"</Text>
    </Flex>
}
```

`Flex::row()` and `Flex::column()` are unstyled `div`-like containers. They
support `gap`, axis alignment, `justify`, wrapping, padding, fixed/fill sizes,
and item behavior (`grow`, `shrink`, `basis`, `align_self`). For the same
chainable properties on any `Style`, import `StyleExt` and start with
`Style::default().flex_row()`, `.flex_column()`, or `.grid()`.

For advanced layout properties, use the re-exported Taffy types through
`creamui::core::layout`.

## Common styles

`creamui::Style` is the component-independent style declaration. It combines
layout, paint, typography, and interaction-state patches; each component uses
only the properties it can render. Interaction patches intentionally exclude
layout so a hover or press cannot move its own hit target.

```rust
use creamui::{ColorToken, StateStyle, Style, StyleProp};
use creamui::widgets::layout::StyleExt;

let action = Style::new()
    .size(220.0, 48.0)
    .background(ColorToken::SurfaceElevated)
    .border(ColorToken::Border, 1.0)
    .corner_radius(8.0)
    .color(ColorToken::TextPrimary)
    .font_size(14.0)
    .hover(StateStyle::new().background(ColorToken::AccentHover))
    .pressed(StateStyle::new().background(ColorToken::AccentPressed))
    .focus(StateStyle::new().outline(ColorToken::Accent, 2.0))
    .property(StyleProp::parse("min-width", "120px").unwrap());
```

Existing `creamui::core::layout::Style` values still work anywhere a common
style is accepted through the `From`/`Into` adapter. This keeps existing Taffy
struct literals valid while applications migrate declarations incrementally.

Strings are accepted only at the declaration boundary through
`StyleProp::parse`. They are immediately compiled into typed `ColorValue` and
`LengthValue` values; widgets and the renderer never interpret CSS strings.
Semantic `ColorToken`s are resolved against the window's current color scheme
at paint time, so a stored style follows theme changes without being rebuilt.

Every widget also implements the `Styled` extension trait automatically. A
component does not need its own `width`, `background`, or `font_size`
forwarders:

```rust
use creamui::core::layout::Style as LayoutStyle;
use creamui::widgets::RawButton;
use creamui::{ColorToken, Styled};

let button = RawButton::new(LayoutStyle::default(), || {})
    .width(120.0)
    .height(40.0)
    .background(ColorToken::Accent)
    .corner_radius(8.0);
```

The first call creates a transparent `StyledWidget`; the rest mutate that
same declaration. Painting, children, measurement, focus, pointer and keyboard
handlers are delegated to the original widget without adding a layout node.
For configuration loaded from strings, use `.property(StyleProp::parse(...)?)`.

Box paint is centralized: the renderer draws the resolved background, border,
radius, and outline before calling a widget's content-specific `paint` method.
Pointer, focus, and component-owned disabled states are composed rather than
being mutually exclusive.

## Next steps

- Browse [components](components.md) for the component families and their state model.
- Read [theming](theming.md) before creating an application-specific visual language.
- Run `cargo run -p showcase` from a clone of this repository to explore live controls.
