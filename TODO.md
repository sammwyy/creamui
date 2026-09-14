# TODO

- GPU upload/draw-call counters (`crates/render/src/gpu.rs`, `perf-metrics`
  feature) are wired but unverified against a live `wgpu` surface — the
  Phase 0 baseline was recorded on a machine with no display server. Run
  `crates/bench` on a machine with a GPU and display attached, then add a
  windowed benchmark exercising `GpuState::present`/`present_partial`, and
  append the results to `docs/performance/baseline.md`.
- `damaged_rect_count` / `damaged_pixel_area` are only populated by the
  windowed animation-damage path in `crates/render/src/window.rs`; no
  headless benchmark exercises them yet.
- `crates/core/src/runtime` (the persistent runtime tree) is not wired
  into `Renderer`/`window.rs` yet — it only exists alongside the old
  reconcile path, reachable via `mount_legacy_widget`, `create_binding`,
  `create_branch`, `create_keyed_list`, and `MountCx`/`View`/`IntoView`.
- The `jsx!` proc macro (`crates/macros`) still expands to widget-builder
  calls, not `MountCx` operations — REFACTOR.md Phase 4's actual
  compiler rewrite (categorizing static/reactive/event/conditional/keyed
  JSX expressions, `#[component]` becoming a mount function, `CREAMUI_DUMP_JSX`
  debug support) is not started. `MountCx`/`View`/`IntoView` exist as the
  target the rewrite should compile into; the rewrite itself needs its own
  pass validated against the full example suite given its blast radius.
- `mount_legacy_widget` always produces `NodeKind::Custom` — it never
  classifies a legacy widget as `Text`/`Image`, so nothing downstream can
  yet tell a runtime-mounted text node from an opaque one without
  widget-specific knowledge.
- `mount_legacy_widget`'s one-time translation cost (1.22s for 50,000
  nodes, see `docs/performance/baseline.md`) is unoptimized; revisit if a
  later phase ends up calling it more than once per mount instead of only
  at initial mount.
- `Owner` (`creamui-reactive`) has no parent back-pointer, so a disposed
  child scope stays in its parent's `children` list until the parent
  itself is disposed — a deferred-reclamation memory-growth concern for
  long-lived parents with many toggles/reorders under them (see
  `docs/performance/baseline.md`'s Phase 3 section), not a correctness or
  subscription-leak issue. Fixing it needs a `Weak` parent back-pointer.
- `Runtime::sync_layout_rects` (`crates/core/src/runtime/mod.rs`) walks
  every node after every `compute_layout` call to find which rects moved —
  O(whole tree), not O(affected branches), since nothing in the `taffy`
  API used here reports which nodes it actually recomputed. Measured at
  ~23.5ms for a 50,000-node tree after a single leaf's style change (see
  `docs/performance/baseline.md`'s Phase 5 section). `taffy`'s own
  recompute is cached and cheap; this post-pass is the remaining cost.
- `RuntimeNode` always gets a `taffy` node at creation
  (`RuntimeTransaction::create_node`); REFACTOR.md 11.1's "not every
  logical runtime node necessarily needs a Taffy node forever" (flattening
  wrapper/component nodes) is not attempted.
- `LayoutState::measure_fingerprint`-based skip only covers the `taffy`
  context write; there is no measurement result cache (REFACTOR.md 11.4) —
  a `taffy` `MeasureFunction` still recomputes from scratch whenever
  `compute_layout` actually calls it.
- `Runtime::rebuild_hit_test` walks every node to find interactive ones —
  O(whole tree), not O(interactive nodes). ~787µs for a 50,000-node tree
  with one interactive leaf (see `docs/performance/baseline.md`'s Phase 6
  section).
- Hit-test entries are collected in plain depth-first child order with no
  z-order/stacking-context handling (REFACTOR.md 12.2) — an absolutely
  positioned node isn't given priority the way the legacy `Scene`'s
  Flow/Absolute two-pass paint does.
- No spatial index (REFACTOR.md 12.4) for hit-testing; `hit_test` is a
  linear scan over `hit_entries`.
- `crates/core/src/runtime`'s event/hit-test state (`EventState`,
  `hovered`/`pressed`/`focused`/`pointer_capture`) is not wired into
  `window.rs`'s pointer/keyboard event handling, same as the rest of the
  runtime tree.
- `Runtime::generate_fragment` only produces primitives for native
  `Container`/`Text`/`Image` nodes; `NodeKind::Custom` (legacy-mounted
  widgets) gets an empty fragment, since `mount_legacy_widget` drops the
  widget after translating it. `RecordingPainter` exists but nothing
  currently calls it post-layout with a still-alive widget.
- No damage-rect merging or full-window collapse above an area/count
  threshold (REFACTOR.md 13.5) — `rebuild_paint`'s damage is a raw list
  of old+new bounds per regenerated fragment.
- No retained clip/opacity grouping (REFACTOR.md 13.4) — `RuntimeNode`
  has no clip or opacity flag, so `PushClip`/`PopClip` aren't emitted for
  clipping containers and there is no opacity property node at all
  (transform now has one — `RuntimeNode::transform`/`effective_transform`
  — see the Phase 10 entries below). `PaintOp::PushTransform`/
  `PopTransform` still exist in the enum but nothing generates them:
  Phase 10 deliberately keeps `effective_transform` out of the `PaintOp`
  stream so a transform-only change never regenerates a paint fragment.
- `crates/render/src/gpu_scene` (REFACTOR.md Phase 8) is not wired into
  `Renderer`/`window.rs` — same "exists alongside, not selectable yet"
  state as `crates/core/src/runtime`. There is no `RenderBackend::GpuScene`
  variant and no caller ever constructs a `GpuSceneState`. Wiring it in
  needs the runtime tree itself wired in first (`GpuSceneState::sync_node`
  takes a `RuntimeNodeId` and `PaintFragment`, both currently reachable
  only off the unwired `Runtime`).
- `GpuSceneState` rasterizes `PaintPrimitive::Quad`/`Border` (via
  `sync_node`) and `Text` (via `sync_text_node`, REFACTOR.md Phase 9);
  `Image` primitives are still silently dropped, and `PushClip`/
  `PushTransform` ops are ignored, so nested clipping/transforms don't
  composite correctly yet. Shadows (14.8 step 6) aren't started.
- `GpuSceneState`'s pipeline, shader, and sRGB decode path are unverified
  against a live surface — same "no display server" constraint as the
  `gpu.rs` counters above. Needs a windowed smoke test (a scene with a
  handful of quads, compared visually or via `GpuState`'s CPU path) on a
  machine with a GPU and display attached before this is trusted.
- `GpuSceneState`'s instance buffer only grows, never shrinks — a scene
  that mounts many quads and then unmounts most of them keeps the larger
  buffer allocated. Same applies to the glyph instance buffer.
- `GlyphAtlas::grow` (doubling the atlas and forgetting every placement)
  is implemented and unit-tested in isolation but `GpuSceneState` never
  calls it — the atlas is a fixed 1024x1024 `R8Unorm` texture, and
  `sync_text_node` silently drops any glyph `GlyphAtlas::place` can't fit.
  Wiring `grow` up for real needs the GPU texture recreated at the new
  size *and* every already-baked `GlyphInstance` in `glyph_store`
  re-derived (their `uv_min`/`uv_max` are baked against the old atlas
  size and go stale the moment `size()` changes) — not attempted here
  since getting that rebake wrong would silently corrupt already-visible
  text.
- `ShapeCache`/`GpuSceneState::sync_text_node` are only reachable from the
  unwired `gpu_scene` path — the live legacy renderer
  (`crates/widgets/src/text_metrics.rs`, `crates/render/src/font.rs`)
  still reshapes text on every measurement and every paint with no cache,
  exactly the "Bad" pattern REFACTOR.md 15.5 describes. Sharing one
  shaping cache between the legacy path and `gpu_scene` needs the legacy
  callers reworked to shape at a canonical origin/alignment and apply
  align/paint-position offsets afterward, the way `sync_text_node` does —
  not attempted here to avoid risking the live rendering path with no way
  to visually verify the change in this environment.
- `sync_text_node` only emits `PaintPrimitive::Quad`/`Border` fill in a
  single block per node — text selection highlighting, underline/
  strikethrough decorations, and per-glyph color runs (the legacy
  `Painter::fill_text_selected` path) have no `gpu_scene` equivalent.
- `TextPrimitive::family`/`bold` are populated for native
  `NodeKind::Text` nodes in `generate_fragment`, but `RecordingPainter`
  (the legacy-widget paint-recording path) still only implements the
  base `Painter::fill_text`, not `fill_text_weight`/`fill_text_font`, so
  a legacy widget's family/bold choice never reaches a recorded
  `TextPrimitive` — consistent with `NodeKind::Custom` already getting an
  empty fragment (see the `mount_legacy_widget` entry above).
- `Runtime::rebuild_composite` (REFACTOR.md Phase 10) and
  `GpuSceneState::sync_transform` exist and are tested/benchmarked in
  isolation, but nothing calls either from a shared place — same
  "exists alongside, not wired into a caller" state as the rest of the
  runtime tree and `gpu_scene`. A real caller would run
  `rebuild_composite` after every transaction and, for each id it
  returns, call `sync_transform(id, runtime.get(id).unwrap().layout.effective_transform)`.
  `Mutation::SetTransform` also has no scroll-view/drag caller yet — it
  exists as plumbing, not wired to any interaction.
- Only translation is modeled (`Transform2D { x, y }`) — no scale/rotate,
  no `OpacityNode`/`ClipNode` (REFACTOR.md 16.1's other two retained
  property kinds), and no explicit compositor-owned layer-promotion
  policy or devtools layer-memory view (16.4/16.5's remaining exit
  criteria). The existing paint-time layer promotion heuristic in
  `crates/core/src/scene.rs` (the legacy reconcile path) is untouched.
- `Runtime::rebuild_composite`'s cascade can revisit the same node twice
  in one pass if a single transaction calls `Mutation::SetTransform` on
  both a node and one of its ancestors — harmless (recomputes to the
  same value, `#[cfg(feature = "perf-metrics")]`'s
  `composite_nodes_updated` counter just double-counts that node), not
  worth a dedup pass for this edge case yet.
- `creamui_core::HeightIndex`/`visible_range` (REFACTOR.md 17.1/17.2)
  are a standalone, tested data structure with no caller — no
  `VirtualList` widget, no wiring to `create_keyed_list`, no scroll-event
  integration. `HeightIndex` only supports append (`push`) and shrink
  (`truncate`); there is no `O(log n)` mid-sequence insert/remove, so a
  list that removes an arbitrary item (not just the last one) needs a
  full rebuild via `HeightIndex::new` today.
- No recycling pool for virtualized rows (REFACTOR.md 17.3) — not
  attempted, per 17.3's own guidance to add one only once allocations
  are measurably a problem, and there is no `VirtualList` yet to measure.
- Virtual table (column virtualization, sticky headers, selection state,
  REFACTOR.md 17.4) and virtual tree (flattening expanded nodes, keying
  by stable tree ID, REFACTOR.md 17.5) are not started.
- `creamui_image::BackgroundImageLoader` (REFACTOR.md 18.1) decodes off
  the UI thread and delivers a `ResourceReady` message, but stops there —
  nothing turns a `ResourceReady` into a runtime mutation or a
  `gpu_scene` texture upload. `NodeKind::Image` only stores a `source:
  Rc<str>`; there is no decoded-pixels field or side-table for a
  `RuntimeNodeId` to receive one, and `gpu_scene` still has no image/
  texture manager at all (the Phase 8 TODO entry above).
- `BackgroundImageLoader` spawns one OS thread per request with no pool
  and no cap — fine for a handful of images, but N simultaneous large
  requests spawn N threads. Revisit if that's ever a real workload.
- Widgets still construct `ImageData` synchronously
  (`ImageData::from_bytes`/`from_path`, used directly by `Image::new`);
  `BackgroundImageLoader` is a separate, opt-in path nothing calls yet.
