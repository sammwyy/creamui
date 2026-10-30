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

## Follow the desktop automatically

Enable the `system-theme` feature and opt the application into live system
following. This applies to queued windows and windows opened later:

```rust,ignore
use creamui::{AppBuilder, WindowOptions, Color};

AppBuilder::new()
    .system_theme()?
    .window(WindowOptions::default(), Color::rgb(0, 0, 0), |_| {}, build_ui)
    .run();
```

`Theme::default()` stays a deterministic dark palette. The loader performs host
I/O; the renderer performs live queries off the UI thread and applies changes
on the UI thread. `WindowOptions::system_theme()` loads once; use the application
builder for continuous following. `WindowHandle::set_theme()` pins that window
to the application's explicit theme. Its host appearance hook keeps updating.
Draw backgrounds from `use_theme().surface` when they should follow the palette;
a caller-supplied fixed `clear_color` remains that color.

The loader resolves in this order:

1. Explicit builder overrides, then `CREAMUI_THEME`, `CREAMUI_VARIANT`,
   `CREAMUI_ACCENT`, `CREAMUI_FONT`.
2. A valid desktop snapshot selected by `CREAMUI_APPEARANCE`, when no explicit
   theme or variant was selected. This preserves the desktop's complete native
   colors, geometry, typography and arbitrary variant ID. Accent, font and
   corners can still be overridden locally. Missing or stale snapshots fall
   back to ordinary resolution.
3. Saved CreamUI preferences, then the selected CreamUI theme or the built-in
   theme. Without an explicit variant, use the host's named variant if present in that theme, otherwise a
   variant matching its light/dark preference, otherwise the theme's default.

Explicit builder/environment variants always win. Without a session snapshot,
a variant saved in `cream/appearance.toml` (`%APPDATA%/CreamUI/appearance.toml`
on Windows) also wins over the host preference. Delete that saved `variant`
key to resume automatic matching. A session snapshot takes precedence over
legacy saved preferences, so an existing Coconut migration keeps working.
An unavailable explicit theme/variant remains a configuration error. An unavailable host
provider is `NoPreference`, so the theme's own default remains usable.
Accent priority is explicit selection, theme default accent, host accent, then
variant accent. Native high contrast is the default unless the CreamUI
accessibility file explicitly overrides it.

## Query host appearance and named variants

```rust,ignore
// Anywhere, including outside a renderer:
let host = creamui::appearance::query();
// query_native() omits a session snapshot (for desktop appearance providers).
let resolved = creamui::theme_loader::SystemThemeLoader::new()
    .load_with_system(&host)?;

// Inside a window's UI scope (even with a fixed application theme):
let host = creamui::use_system_appearance(); // Signal<SystemAppearance>
let preference = host.get();              // reactive read
if preference.variant.as_deref() == Some("midnight") {
    // The provider specifically published Midnight.
}
```

`SystemAppearance` contains `color_scheme` (`NoPreference`, `Light`, `Dark`),
optional provider `variant`, optional accent, high contrast, reduced motion and
`source`. `use_theme()` gives the effective app palette; the host hook describes
the desktop independently of application overrides. Read the hook in a reactive
build/effect, or obtain its signal there and keep it for later reads.

Linux queries the **public XDG Settings portal** and listens for
`SettingChanged`. When unavailable, it queries GNOME GSettings, with a bounded
subprocess timeout and periodic reconciliation. Windows queries WinRT
`UISettings`, subscribes to `ColorValuesChanged`, and reads native high contrast;
a read-only `AppsUseLightTheme` registry query is its fallback. Native fallback
queries run approximately every two seconds; session and config file replacement
is checked approximately every 200 ms. Dropping a listener stops its worker.
Unsupported platforms return `NoPreference` unless a desktop snapshot supplies
a preference. These APIs do not require GTK or Qt and do not change host settings.

The standard light/dark preference does not describe a full GTK/Qt theme and
cannot identify Midnight. CreamUI retains a provider's arbitrary named variant
separately. Coconut exports it in its native snapshot and in the Settings
namespace `org.creamui.appearance`, key `variant` (string); the standard
`org.freedesktop.appearance.color-scheme` remains compatible with other toolkits.
Other desktop providers may publish that extension too. Snapshot publishers can
use `appearance_snapshot_with_scheme()` to declare a named variant's light/dark
classification explicitly. Older snapshots without `color_scheme` remain readable.

A CreamUI theme can classify its own arbitrary names for automatic matching:

```toml
[variants.latte]
appearance = "light"
# [variants.latte.colors] ... complete palette

[variants.oled]
appearance = "dark"
# [variants.oled.colors] ... complete palette
```

The `appearance` field is optional. Existing `light`, `dark` and `midnight` names
retain their built-in classification. Other names without metadata only match
an explicitly published variant ID; they are never guessed from a theme name.
When several variants match a mode, prefer the theme's default, then the
canonical `light`/`dark` variant, then the first matching ID in sorted order.

For headless consumers, use the `creamui-appearance` crate directly and
`creamui-theme-watch::ThemeListener` for reloads. The `portal` feature in the
appearance/loader crates and `dbus` in the watcher are enabled by default for
direct users; disable default features in all three to omit D-Bus from Linux.

Native protocol references: [XDG Settings](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html)
and [Windows themes](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-windows-themes).
