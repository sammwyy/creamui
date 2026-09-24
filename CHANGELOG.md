# Changelog

All notable changes to CreamUI will be documented in this file.

## Unreleased

- Animations follow the display: after a frame reaches a presenter or
  platform that holds the next one until the display refreshes (a GPU
  surface, or winit on Wayland) the next frame is requested right away;
  elsewhere a timer runs at the monitor's reported refresh rate instead of
  a fixed 16 ms. Every widget in a frame reads the same animation time.
  `PlatformWindow` gained `paces_redraws` and `refresh_interval`.
- Scroll views record their content as a scroll layer
  (`Painter::push_scroll_layer`/`pop_scroll_layer`) in its own content
  space. The frame diff (`display_list::diff`) aligns rows entering and
  leaving the viewport, and when one layer moved by whole pixels over a
  solid backdrop the CPU rasterizer shifts its pixels
  (`Rasterizer::scroll`/`apply`) and repaints only the exposed strip; the
  GPU applies layer offsets in the vertex shader.
- A clipping container passes its own rect to `Painter::push_clip_rounded`
  instead of the rect already cut by enclosing clips, so its rounded corners
  stay on its own edges when it is partly scrolled out of view.
- Fixed partial CPU repaints antialiasing rounded corners differently from
  full ones where a corner crossed the damage boundary.
- The GPU renderer uploads only the instances, clip table and globals a
  frame changed, finds a glyph's atlas slot without hashing, and uses
  FxHash for the text and clip caches.
- Text layouts no longer depend on their box height, so resizing a box
  vertically reuses the layout (`TextSystem::layout` lost its `height`
  parameter; `TextLayout::height` is the block height to center).
- `Runtime::compute_layout` only syncs rects along changed nodes' ancestor
  paths and subtrees that moved, and `Runtime::rebuild_hit_test` patches
  moved entries in place, rebuilding only when membership changes.
- Startup: the GPU adapter and device are requested on a background
  thread while the first window is created, pipelines use a driver
  pipeline cache persisted under the user cache directory, and the system
  font index is cached there too, validated by directory modification
  times.

- Replaced `fontdue` with `swash`: fonts are memory-mapped and parsed on
  demand instead of copied and fully decoded, and text is shaped (kerning,
  ligatures, per-script runs) and broken at Unicode line-break
  opportunities by `creamui_fonts::layout`, shared by measurement and
  painting. `FontFace` is now `creamui_fonts::FontFace`.
- Fixed `creamui_fonts::resolve` returning a family's regular face for bold
  requests once the regular one had been loaded, so system bold faces were
  never used.
- GPU windows on one device share the shader, pipelines, glyph atlas and
  image textures (`GpuShared`); `GpuRenderer::new` takes the shared
  resources. Instances shrank from 128 to 56 bytes by packing colors and
  moving clips into a per-frame lookup texture. A full glyph atlas is
  cleared of stale glyphs before it grows and shrinks again when mostly
  unused, and the instance buffer shrinks after large frames.
- Images are uploaded downscaled to the size they are drawn at, and
  `ImageData` loaded from bytes or a path drops its decoded pixels after
  upload, decoding again only if they are needed. `RgbaImage::pixels`
  returns an `Arc<[u8]>`; `RgbaImage::reloadable` and
  `RgbaImage::discard_pixels` were added.
- The text layout and glyph caches are shared by every window on the UI
  thread.

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
