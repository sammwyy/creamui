# Theming

`Theme::default()` is CreamUI's single default visual language: rounded surfaces, filled selection, balanced spacing, and a dark color scheme.

`Theme::light()` keeps the same component geometry and typography while using a pale canvas and white cards. `Theme::dark()` uses neutral charcoal surfaces, and `Theme::midnight()` uses deeper, near-black surfaces. All three share the same default cyan accent.

The palettes live in `ColorScheme`, so application backgrounds, controls and system theme loading use the same tokens. Their surface alpha values support translucent windows; applications should paint the canvas once rather than stacking the same translucent background on every container.

```rust
let theme = Theme::default();
let light_theme = Theme::light();
let midnight_theme = Theme::midnight();
```

## Customize tokens

`Theme` is a regular copyable value. Start from the default and adjust the tokens that matter to your application.

```rust
let mut theme = Theme::light();
theme = theme.with_accent(Color::rgb(72, 117, 255));
theme.card_radius = 16.0;
theme.spacing_large = 20.0;
```

Themed widgets read their colors, typography, spacing, and component radii from the value you pass them. Keep a single theme in application state when you want a live appearance switcher.

## Use raw widgets for exceptions

Use `Raw*` widgets for a component that intentionally does not follow your main theme. This keeps special visual treatments explicit rather than adding one-off rules to every themed component.
