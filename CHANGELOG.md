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
- Added an opt-in `perf-metrics` feature to `creamui-devtools`: the F3
  overlay grows an "engine" panel of reconcile/layout/paint/GPU counters
  fed by `creamui-core`'s existing `FrameMetrics`.
- Added `RawVirtualList`/`VirtualListState`: a scrollable list that mounts
  only the rows visible in its viewport (plus overscan) instead of every
  row, built on `creamui-core`'s existing `HeightIndex`.
- Fixed a quadratic cost in `RuntimeTransaction` for a long-lived
  transaction touching many distinct nodes.
- Merge overlapping animation-tick damage rects (and collapse to a full
  repaint above a size/count threshold) before partial-presenting a
  frame, instead of uploading each one separately.
- Fixed `Owner` (`creamui-reactive`) leaking a disposed child scope's slot
  in its parent's child list until the parent itself was disposed.
- Added `cui_set_transform`, `cui_compute_layout`, and `cui_get_rect` to
  ABI-v2, so a C caller can trigger layout and read back a node's
  computed window-space rect instead of only mutating the tree blind.
- Added `Widget::legacy_node_kind`, letting a legacy widget mounted onto
  the persistent runtime tree report itself as `NodeKind::Text` instead
  of an opaque `NodeKind::Custom`. Implemented by `RawText`/`RawPre`/
  `RawLink` and their themed wrappers.
- Cache a leaf's measurement result within one layout pass instead of
  recomputing it every time `taffy`'s flex/grid algorithm re-queries the
  same node with identical inputs.

## 0.1.1

- Added the optional `creamui-devtools` crate with an FPS, frame-time, CPU,
  and RAM overlay toggled with F3.
- Added the `devtools` feature to the `creamui` facade.
