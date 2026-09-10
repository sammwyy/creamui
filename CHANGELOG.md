# Changelog

All notable changes to CreamUI will be documented in this file.

## Unreleased

- Replaced the public layout-only widget style contract with CreamUI's common
  `Style`, covering layout, paint, typography, and stable interaction-state
  patches. Existing `creamui_core::layout::Style` declarations remain accepted
  through conversion adapters.
- Replaced button-specific public style types with the shared `StateStyle`
  model and migrated raw buttons, views, and text to consume the common
  properties they understand.
- Added typed `ColorValue`, `ColorToken`, `LengthValue`, and `StyleProp`
  declarations, including CSS-like parsing at the API boundary and runtime
  theme-token resolution.
- Centralized box painting in the renderer and replaced mutually exclusive
  interaction states with composable `StyleState` flags. Foundational raw
  widgets now keep one style declaration instead of mirrored visual fields.
- Added the blanket `Styled` extension API, giving every widget common typed
  builders without per-component forwarding methods or extra layout nodes.
  Migrated all raw widget style storage and common paint/typography fields to
  the shared declaration; remaining visual fields are component-specific.

## 0.1.1

- Added the optional `creamui-devtools` crate with an FPS, frame-time, CPU,
  and RAM overlay toggled with F3.
- Added the `devtools` feature to the `creamui` facade.
