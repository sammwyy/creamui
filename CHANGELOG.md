# Changelog

All notable changes to CreamUI will be documented in this file.

## Unreleased

- Replaced CPU-raster-then-upload rendering with a retained display list:
  widgets record primitives, consecutive frames are diffed into damage,
  and only changed pixels are redrawn and presented. The GPU backend now
  draws rounded rects, borders, lines, glyphs and images with one instanced
  SDF pipeline and falls back to the CPU backend when no adapter works.
- Removed the per-widget pixel layer cache (`Painter::push_layer` and
  friends, `Widget::paint_fingerprint`, `Widget::paints_transparently`),
  which pasted stale backdrops and allocated window-sized clip masks.
- The CPU backend presents through an `Argb8888` `wl_shm` buffer on Wayland
  and passes alpha to `softbuffer` elsewhere, so transparent windows work
  without a GPU.
- Text layouts and glyph bitmaps are cached across frames.
- Resizes lay out on the next frame instead of after a 100 ms settle delay,
  clicks and key presses no longer render synchronously, and widgets
  scrolled out of their clip are not painted.
- `Painter::draw_rgba_image` became `Painter::draw_image`, taking a shared
  `RgbaImage` and an optional tint; `IconImage` holds an `RgbaImage`.
- Keyboard focus no longer jumps to another widget when a focusable one is
  scrolled out of view.
- Devtools report per-stage frame timings, damage and cache sizes, and
  `CUI_FRAME_LOG=1` prints them per frame. Added the `gpu` benchmark and
  the `frame_report` example.

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
- Fixed the persistent runtime tree's hit-testing to give an absolutely
  positioned node (a popover, an overlay) priority over a later flow
  sibling, regardless of tree depth or document order.
- Fixed `RecordingPainter` dropping a legacy widget's bold/font-family
  choice instead of recording it.
- Added `cui_set_typography_style` to ABI-v2, so a C caller can set a
  node's color/font-size/family/align/bold/italic/underline/strikethrough.
- Added retained per-node opacity to the persistent runtime tree,
  cascading multiplicatively to children alongside the existing
  transform compositing.
- Added retained per-node clipping to the persistent runtime tree,
  intersecting each clipping ancestor's own rect into its children's
  effective clip region.

## 0.1.1

- Added the optional `creamui-devtools` crate with an FPS, frame-time, CPU,
  and RAM overlay toggled with F3.
- Added the `devtools` feature to the `creamui` facade.
