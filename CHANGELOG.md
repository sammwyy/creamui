# Changelog

All notable changes to CreamUI will be documented in this file.

## Unreleased

- Pointer and touch focus no longer draw keyboard focus outlines on controls.
  Keyboard navigation restores the indicator; text fields retain their caret
  and focus appearance after pointer activation.
- Checkboxes and switches animate checked-state changes with a configurable
  160 ms ease-out transition. Switch thumbs slide as track colors blend;
  checkbox fills fade and checks reveal progressively. Renderer-owned
  `TransitionState` survives rebuilds and keyed reordering, reverses without
  jumps, and requests frames only while moving. `Transition::NONE` disables
  animation, and custom widgets can use the same transition hooks.
- `View::new` defers mounting until a `MountCx` is available. `MountedView`
  owns a stable runtime root and binding scope, releases both on disposal or
  drop, and cleans up a partially mounted view after a panic. Deferred and
  compatible widget views respect detached root mounting. Removing roots or
  detached subtrees now notifies runtime observers.
- Normal window, popup, and dynamic-window builders accept retained `View`
  results alongside widget results. Retained views mount once inside the
  window's hook context, react directly to signals, theme, and viewport, and
  dispose on close even when handles retain the pipeline or runtime. Surface
  suspension keeps the mounted view. Failed mounts use the registered panic
  handler and release partial nodes before displaying an empty fallback.
- `CREAMUI_DUMP_JSX` emits generated Rust for native JSX, ABI JSX, and component
  declarations. Cargo tracks changes to the setting; empty and `0` values
  disable output.

- Retained branches and keyed lists preserve their position among static
  siblings with hidden anchors, without introducing layout wrappers or gaps.
  Derived conditions and list sources are supported. Surviving keyed rows receive
  `Signal<T>` values, preserving their nodes, bindings, and event handlers when
  data changes; row values now require `PartialEq`.
- Runtime mounting attaches keyed roots in one batch, and subtree removal
  unlinks each surviving parent once. Overlapping roots and deep trees are
  supported without recursive removal.
- Reactive effects preserve their mounting context, support untracked reads,
  coalesce nested notifications, and cancel queued work on disposal. Owner
  disposal stops all descendant effects before child-first cleanup, batches
  sibling detachment, and rejects new work in disposed scopes. Non-owning scope
  handles avoid retaining parent owners through structural effects.
- Retained composition tests cover sibling order, flex spacing, updated row
  events, duplicate keys, reentrant cleanup, disposal during mounting, and
  10,000-node trees. Keyed mount, reorder, and removal benchmarks exercise
  1,000, 10,000, and 50,000 bound rows; shared-signal benchmarks cover the
  same subscriber counts.

- Android supports CPU presentation with direct premultiplied RGBA copies into
  native buffers, including the expanded damage returned by the platform.
  GPU startup can fall back to the CPU presenter.
- Platform suspension reaches the render loop. CPU and GPU windows release
  native surfaces, cancel pointer gestures, and stop frame timers while
  suspended. Resume recreates the presenter, refreshes scale and viewport,
  preserves reactive state, and forces a full frame without repeating the
  window-ready callback.
- Android supports one activity window. Extra window and native popup requests
  log a warning without calling their ready callbacks; hosted overlays provide
  secondary views. Closing a window releases its platform ID so an application
  that keeps running can create a replacement.
- Failed presentation retries repaint the complete frame, preventing repeated
  CPU scroll shifts. Android lifecycle fixtures check both backends, partial
  damage, repeated suspension, rotation, retained state, input, and window
  rejection. CI checks Android examples and fixture compilation.

- Browser file pickers open asynchronous native selection dialogs, apply
  extension filters, and release their DOM handlers on selection or cancel.
  `FilePicker::new` callbacks now receive `SelectedFile`, with a display name,
  an optional native path, and deferred asynchronous `read_bytes()` on every
  platform. Native reads run off the UI thread.
- Browser text inputs and text areas use the asynchronous clipboard API.
  Controlled editors preserve pending pastes across rebuilds and discard
  results after newer edits, navigation, or paste requests. Change guards
  apply to pasted text, and caret/selection indices stay on UTF-8 boundaries.
- Text areas preserve queued keyboard edits before the next reactive rebuild.
  Browser integration tests cover file dialogs, cleanup, clipboard access,
  denied reads, and delayed paste responses.

- Legacy runtime mounting batches each container's child attachment, avoiding
  repeated writes of growing sibling lists while preserving child order.

- Retained layout records changed Taffy outputs and rounds only affected
  paths and descendants whose cumulative position changed. Rectangle sync
  skips unchanged flow siblings in wide containers, while preserving layout,
  clipping, and hit regions after resizing, hiding, or reparenting nodes.
  Devtools exposes the number of nodes rounded per frame.

- Android `FilePicker` opens a themed local path modal through a persistent
  `FilePickerController` host. It preserves edits, validates readable regular
  files and extension filters, and keeps background controls outside modal
  pointer and focus navigation. Dialogs share that isolation. Themed text
  inputs forward IME eligibility, and opening a modal enables the keyboard.
- File pickers expose their themed box styles to the scene painter and retain
  common style overrides. Narrow showcase cards stack vertically.
- Android inset and system-bar calls use the window's actual Activity;
  system-bar mutations run on the Java UI thread. The platform
  `sync_system_bars` function now requires a `PlatformWindow` argument.
  Unicode control keys retain Enter, Tab, Backspace, and Escape semantics;
  physical modifier keys enable selection and clipboard shortcuts. Insets are
  sampled through keyboard transitions so dismissal restores the full viewport.
- Android GPU devices discard native object labels to avoid Ranchu's Vulkan
  debug-name crash. The Android demo writes framework diagnostics and panics
  to logcat.

- GPU windows retain a color texture and redraw calculated damage, clearing
  translucent regions before replay and culling unrelated instance ranges.
  Each swapchain buffer receives the complete retained frame by texture copy
  or fullscreen blit, without relying on buffer age. Resize forces a full draw.
  Devtools reports GPU pixel and instance counts; headless rendering exposes
  the same retained path for pixel checks.
- GPU atlas pressure reclaims cold glyph shelves in batches, preserving hot
  glyph coordinates and uploads. Reused slot handles validate their glyph key,
  and cleared shelf padding keeps filtered samples isolated.
- Devtools adds a retained-tree inspector with stable node selection and
  invalidation reasons. F4 toggles the tree, F5 damage, F6 layout rectangles,
  and F7 hit regions; F8 and Shift+F8 browse nodes. Damage reflects content
  display-list changes, including transforms and removal, without including
  the overlays. Runtime snapshots and invalidation tracking are opt-in.
- `use_system_bars()` is exported by the render and facade crates.
- Standalone Wayland builds omit the Winit dependency.
- Runtime handler changes update hit membership directly. Paint-order changes
  update ranks while preserving spatial entries, and focus changes rebuild
  only focus order. Removing the runtime root clears retained hit and focus state.
- ABI-v2 retained trees can run in native windows through `cui_run_window` or
  `RuntimeTree::run_window`. C mutations schedule frames in the same runtime
  used for layout, painting, and input.
- Radial backgrounds accept any number of positioned or implicit color stops.
  CPU and GPU rendering interpolate them with premultiplied alpha; repeating
  gradients use the interval between their first and last stops. ABI-v2 paint
  snapshots expose the complete resolved stop list.
- Two-stop radial backgrounds can repeat across each radius on the CPU and GPU,
  including elliptical shapes and translucent colors.
- Radial backgrounds support farthest-corner ellipses and explicit pixel radii
  in typed styles and CSS-like background strings. CPU and GPU rendering use
  independent horizontal and vertical radii.
- Absolute runtime nodes accept a z-index mutation. Paint and pointer hit order
  share its stacking order, while keyboard focus keeps document order.
- Retained layout sync follows changed absolute-child paths without checking
  unrelated siblings. Devtools reports how many layout rectangles were checked.
- `use_safe_area()` reports the logical-pixel bands covered by the status bar,
  navigation bar, display cutout, and on-screen keyboard. Android reads
  `WindowInsets` together with the native content rectangle; the web backend
  reads `env(safe-area-inset-*)`. Desktop client areas report zeros.
- Absolutely positioned layers paint their absolutely positioned descendants,
  so a modal backdrop can hold another floating layer. The showcase drawer
  uses that, leaves a dismiss strip on narrow windows, and insets its top bar
  and navigation by the safe area.
- Runtime-backed windows now paint retained nodes through `Renderer`, respond
  to pointer, keyboard, drag, and scroll input, and redraw after mutations or
  theme changes. Decoded image fit and crop are verified through the CPU
  presenter; clipped translucent images are checked against the GPU renderer.
  A `MountCx` example presents a native CPU window through the retained path.
- Runtime window handles can apply `ResourceReady::into_mutation` results on
  the UI thread, scheduling the retained presenter after decoded images arrive.
- Retained layout sync no longer allocates a child list for every visited
  container while walking changed layout paths.
- The custom Wayland backend reports the current output refresh interval for
  each window, updating it when display modes or surface outputs change.
- GPU image textures now carry box-filtered mip levels and use trilinear
  sampling when a cached image is drawn smaller later.
- Headless GPU tests now serialize device use, avoiding a concurrent-driver
  crash while the rest of the render test suite runs in parallel.
- Added `VirtualTreeView` with flattened expanded rows, stable node keys,
  keyboard selection, and viewport-sized widget materialization.
- Added `RawVirtualTable` and themed `VirtualTable` with fixed headers,
  selection callbacks, and row data loaded only for the visible range.
- GPU instance uploads can cover separate changed ranges, avoiding uploads of
  unchanged rows between changes near the ends of a scrolling list.
- Nested rounded clips now intersect in both CPU and GPU rendering, including
  rounded ancestors of scroll layers.
- The web showcase registers a bundled font before rendering and retains its
  theme effect after the browser event loop returns. Chromium verification
  covered canvas rendering, theme changes, navigation, and text input.
- C callers can request compositor blur at window creation through
  `CWindowOptionsV2` and the versioned run and builder entrypoints. The dynamic
  Rust client exposes the same option while the original C ABI remains intact.
- Reviewed runtime node allocation: branches and keyed lists attach their
  content directly to a parent, while every current node kind participates
  in layout. There are no layout-free nodes whose Taffy allocation can be
  removed yet.
- Runtime pointer hits use a spatial index that preserves paint order and
  updates moved hit regions without rebuilding the full hit list.
- Added a themed `VirtualList` and a 100,000-row example with mixed row
  heights and a draggable scrollbar.
- `RawVirtualList` reads its resolved viewport height and measured row heights
  after layout. Window rendering rebuilds until those measurements stabilize,
  so flex-sized lists use their actual visible range in the same frame. Wheel
  scrolling rebuilds virtualized rows while ordinary scroll views still
  repaint without rebuilding layout.
- Text shaping selects another installed font per missing glyph cluster, and
  the CPU/GPU glyph caches rasterize it with that face. Registered font
  changes invalidate the affected layout caches.
- Text layout applies Unicode bidirectional visual ordering after line
  breaking while keeping source-byte positions stable for editing.
- Measured the conditional `HeightIndex` middle-insert requirement against
  current consumers; no dynamic list caller exists, so the compact Fenwick
  index remains unchanged until such a workload appears.
- `ResourceReady::into_mutation` now converts successful background image
  decodes into `Runtime` image mutations, preserving fit and paint invalidation.
- ABI-v2 now exposes color-scheme driven paint-fragment readback and C click
  callbacks with userdata; `creamui-dynamic` owns a `RuntimeTree` wrapper for
  the same retained tree operations.
- C callers can request or clear compositor blur on a live window handle with
  `creamui_window_set_blur`, including a window-wide or rectangular region.
- Background image decoding now uses a bounded queue and reusable workers,
  avoiding one unbounded thread per request.
- `creamui-dynamic::RuntimeTree` now covers node counts, layout/paint/text
  mutations, layout, retained paint readback, and click dispatch.
- ABI paint text readback now carries an explicit UTF-8 byte length instead of
  assuming the retained string is NUL-terminated.
- `RawVirtualList` uses a fixed height declared in its style as the viewport;
  the explicit viewport remains a fallback for auto-sized lists.
- Keyboard focus follows a retained widget through keyed sibling insertions
  and removals; `keyed(widget, key)` lets existing controls opt into stable
  identity. Removing the focused widget clears focus.
- The persistent runtime retains decoded image pixels and legacy `Custom`
  widgets in paint fragments. Recorded text keeps italic and selection data;
  image fragments keep fit geometry and clipping. `ImageFit::None` now anchors
  the original pixels at the top-left as documented.
- `creamui-platform` now compiles for `wasm32-unknown-unknown`: its winit
  adapter shares one handler implementation between borrowed and owned
  handlers, and its control-flow clock matches `web_time` on the web target.
- Widget measurement and rendering now reuse a bounded, thread-local shaped
  text layout cache in `creamui-fonts`; rendering keeps its separate glyph
  bitmap cache.
- Text inputs and text areas place text, selections, carets, and pointer
  selection within their layout-resolved padding and borders. Themed defaults
  now declare their spacing as style padding.
- CPU linear gradients with different stop alpha values now interpolate
  premultiplied colors, matching the GPU through transparent stops.
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
