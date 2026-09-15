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
- `mount_legacy_widget` now classifies a widget as `NodeKind::Text` when it
  overrides the new `Widget::legacy_node_kind` hook (`None` by default,
  still mounting as opaque `NodeKind::Custom`). Overridden by every raw
  widget whose entire content is one painted text string — `RawText`,
  `RawPre`, `RawLink` — and forwarded by their themed wrappers (`Text`,
  `Heading`, `Pre`, `Link`). Not extended to widgets that only contain
  text as a child (`RawQuote`/`Quote`, buttons, etc. — their `RawText`
  child gets classified on its own via the normal recursive mount) or to
  editable text (`RawTextInput`/`RawTextArea` — `NodeKind::Text` has no
  cursor/selection/editing concept, so classifying them as one would be a
  lossy, misleading mapping). `Image` (`crates/image/src/lib.rs`) doesn't
  override it either: `NodeKind::Image` stores a `source: Rc<str>` (an
  unresolved reference for an async loader, matching
  `BackgroundImageLoader`'s `ResourceReady` flow — see the Phase 12
  entries below), while `Image` the widget already holds fully-decoded
  `ImageData` pixels with no string identifier at all — classifying it as
  `NodeKind::Image` would need either a source string `Image` doesn't
  have or a decoded-pixels field `NodeKind::Image` doesn't have; not
  attempted here rather than force a lossy mapping. Nothing downstream
  (`generate_fragment`, devtools) reads `NodeKind` off a legacy-mounted
  node yet — this only makes the information available.
- `mount_legacy_widget`'s one-time translation cost (1.22s for 50,000
  nodes, see `docs/performance/baseline.md`) is unoptimized; revisit if a
  later phase ends up calling it more than once per mount instead of only
  at initial mount.
- Fixed: `Owner` (`creamui-reactive`) used to have no parent back-pointer,
  so a disposed child scope stayed in its parent's `children` list until
  the parent itself was disposed — a deferred-reclamation memory-growth
  concern for long-lived parents with many toggles/reorders under them
  (see `docs/performance/baseline.md`'s Phase 3 section). `OwnerInner`
  now carries a `Weak<OwnerInner>` back to its parent (never strong, so no
  keep-alive cycle), and `dispose` removes itself from the parent's
  `children` immediately via `Rc::ptr_eq`. `runtime::branch`/`keyed` (the
  two callers that toggle/reorder `Owner::child()` scopes repeatedly)
  weren't otherwise touched — they already call `dispose()` on removal,
  so they get the fix for free.
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
- REFACTOR.md 11.4: `Mutation::SetMeasure` (`crates/core/src/runtime/transaction.rs`)
  now wraps an incoming measure closure with `memoize_measure` before
  storing it as the `taffy` node context — a repeat call with the exact
  same `(known_dimensions, available_space)` pair (common within one
  `compute_layout` pass, e.g. `taffy` resolving flex-basis before its
  final pass) returns the cached `Size` instead of recomputing. Only the
  single most recent call is remembered, not a general result cache
  keyed by every distinct input seen — a leaf is not meaningfully queried
  with more than a couple of distinct inputs within one pass, so a bigger
  cache wasn't worth the eviction-policy complexity. `LayoutState::measure_fingerprint`
  still only gates the `taffy` context *write* (unchanged — see the
  `setting_the_same_measure_fingerprint_twice_skips_the_second_write`
  test), a separate, already-existing optimization from this one.
- `Runtime::rebuild_hit_test` walks every node to find interactive ones —
  O(whole tree), not O(interactive nodes). ~787µs for a 50,000-node tree
  with one interactive leaf (see `docs/performance/baseline.md`'s Phase 6
  section).
- REFACTOR.md 12.2: `Runtime::rebuild_hit_test` (`crates/core/src/runtime/events.rs`)
  now mirrors the legacy `Scene`'s deferred Flow/Absolute two-pass paint
  for `hit_entries` — every normal-flow node is collected first, then
  every absolutely positioned subtree's nodes (in encounter order,
  appended after), so `hit_test`'s reverse scan always picks an
  absolutely positioned node over a flow sibling regardless of tree
  depth/document order. A node nested inside an already-deferred absolute
  subtree isn't independently re-deferred — its whole ancestor subtree
  already moved as one unit (simpler than the legacy renderer's own
  per-level Flow/Absolute mode-switching, and correct for a doubly-nested
  absolute where the legacy path silently drops the paint). `focus_order`
  is untouched — plain depth-first document order, since tab order
  doesn't follow paint order. No stacking-context/z-index concept beyond
  "absolute or not" — two absolutely positioned nodes at the same tree
  level are ordered by encounter order, not an explicit z-index.
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
- REFACTOR.md 13.5: added `creamui_core::merge_damage`/`merge_damage_default`
  (`crates/core/src/damage.rs`) — merges overlapping rects into their
  union and collapses to one full-viewport rect above a rect-count or
  damaged-area threshold. Wired into the one live consumer of raw damage
  rects, `crates/render/src/window.rs`'s animation-tick partial-present
  path (`animated_damage` → `present_partial`); a viewport-covering
  collapse now routes through the existing full `present()` call instead
  of a single full-size `present_partial` region. Not wired into
  `Runtime::compute_layout`/`rebuild_paint`'s own raw per-node damage
  output (`crates/core/src/runtime/mod.rs`), since neither is reachable
  from `window.rs` yet (see the runtime-tree entry above) and their
  existing tests assert exact `damage.len()` counts against the raw,
  unmerged list. No headless test exercises the `window.rs` wiring
  itself — `WindowEventHarness` constructs `presenter: None`, and
  `animated_damage` is populated only via `repaint_animated`'s
  cached-layer animation-tick path, which no existing test drives;
  `merge_damage` itself has full unit coverage in isolation.
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
- `RecordingPainter` (`crates/core/src/runtime/paint.rs`) now implements
  `fill_text_weight`/`fill_text_font` directly, so a legacy widget's
  family/bold choice reaches a recorded `TextPrimitive` instead of
  silently dropping through `Painter`'s lossy default chain down to
  `fill_text`. `RecordingPainter` itself is still not called from
  anywhere live — `mount_legacy_widget` drops the widget after
  translating it, so there's nothing yet that constructs one against a
  still-alive widget post-layout (see the `Runtime::generate_fragment`
  entry above, unchanged by this). `fill_text_selected`/
  `fill_text_selected_font` (selection highlighting) and `italic` still
  fall through the lossy default, since `TextPrimitive` has no field for
  either — a larger change than this one, and already covered by the
  `sync_text_node` entry below for the (also unwired) `gpu_scene` path.
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
- REFACTOR.md 17.1: `creamui_widgets::RawVirtualList` (`crates/widgets/src/raw/virtualize.rs`)
  now mounts only the `VirtualListState`-backed visible range (plus
  `overscan`) by wrapping `RawScrollView` and absolutely positioning each
  visible row inside a full-height spacer, instead of materializing every
  row. It is built on the legacy `Widget` tree (`children()` runs before
  layout), not the persistent runtime tree, since that tree is still
  unwired from `window.rs` (see the entry above) — `viewport_height` is
  therefore a caller-supplied value, not read back from the widget's own
  resolved layout rect. No themed wrapper or example uses it yet.
- REFACTOR.md 17.2: `VirtualListState::set_height` supports per-index
  variable heights (backed by `HeightIndex`), but nothing measures a
  mounted row and calls it back — there is no "row reports its real
  height after layout" feedback path, since the legacy `Widget` trait has
  no post-layout hook a caller could use for that. Every row uses its
  `estimated_height` (or whatever `set_height` was called with) as its
  final height. `HeightIndex` itself still only supports append (`push`)
  and shrink (`truncate`); no `O(log n)` mid-sequence insert/remove, so a
  list that removes an arbitrary item (not just the last one) needs a
  full rebuild via `HeightIndex::new`/`VirtualListState::set_item_count`.
- No recycling pool for virtualized rows (REFACTOR.md 17.3) — not
  attempted, per 17.3's own guidance to add one only once allocations
  are measurably a problem.
- Virtual table (column virtualization, sticky headers, selection state,
  REFACTOR.md 17.4) and virtual tree (flattening expanded nodes, keying
  by stable tree ID, REFACTOR.md 17.5) are not started; `RawTable`/
  `TreeView` still mount every row/node.
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
- ABI-v2 (`crates/ffi/src/runtime.rs`, `cui_*` functions over
  `creamui_core::runtime::Runtime`) now also covers `set_transform`,
  `compute_layout`, `get_rect`, and `set_typography_style`. Added
  `CRect`/`CTypographyStyle` to `creamui-abi` — a caller can trigger a
  real layout pass, read back a node's window-space rect, and set
  color/font-size/family/align/bold/italic/underline/strikethrough, not
  just mutate the tree blind. `CTypographyStyle`'s fields are each
  independently "unset"-able (`has_color`/negative `font_size`/null
  `font_family`/`TEXT_ALIGN_UNSET`/`TRISTATE_UNSET`), mirroring
  `TypographyStyle`'s own `Option<T>` fields — `CTypographyStyle::unset()`
  starts from all-unset. `color` is always a literal `CColor`, never a
  theme token, matching `cui_set_background`'s existing restriction.
  Still no `rebuild_paint`/paint-fragment readback (needs a `ColorScheme`
  representation on the C side, a larger surface than layout/transform/
  typography), and no way to attach event handlers
  (`Mutation::SetEventHandlers` takes an `EventState` of Rust closures,
  which has no C-safe shape yet — would need `extern "C" fn` + userdata
  callbacks the way ABI-v1's `creamui_button_new` already does).
- `creamui-dynamic` (the ABI consumer) still only speaks ABI-v1 — it has
  no `cui_*` bindings and doesn't construct a `CRuntime`. REFACTOR.md
  19.2's "keep ABI-v1 working until dynamic runtime migrates" is
  satisfied by construction (ABI-v2 is new and additive), but the
  migration itself hasn't started.
- No window/present loop reachable from ABI-v2 — it only mutates a bare
  `Runtime` in memory. Turning that into pixels on screen still means
  going through ABI-v1's `creamui_run`, which builds a `BoxedWidget`
  tree, not the persistent runtime tree ABI-v2 mutates.
- Fixed: `RuntimeTransaction::touch` used to dedup its `touched` list with
  `Vec::contains` — `O(n)` per call against however many distinct nodes
  that transaction had already touched, found to dominate a long-lived
  transaction across many mutations while writing `crates/bench/benches/ffi.rs`.
  It now stamps each `RuntimeNode` with the owning transaction's
  monotonically increasing `Runtime::transaction_stamp` on first touch
  (`RuntimeNode::touched_stamp`), so a repeat touch is an `O(1)` field
  compare instead of a scan. The `Vec::contains` fallback stays only for
  the rare case where `touch` is called with an id that no longer
  resolves to a live node (nothing to stamp).
- REFACTOR.md Phase 14 (20.3 only): `creamui-devtools`'s F3 overlay now
  shows an "engine" panel of `creamui_core::metrics::FrameMetrics`
  counters (reconcile visits, taffy writes, layout/measure calls, paint
  visits/records, hit/composite updates, damage, GPU upload bytes, draw
  calls) behind its own `perf-metrics` feature, and `crates/render/src/window.rs`
  now resets/reads that thread-local once per full frame instead of
  never — it was previously only exercised by tests. The rest of Phase
  14 is not started: 20.1 (runtime tree inspector: `RuntimeNodeId`,
  `NodeKind`, dirty flags, compositor layer, per-node) and 20.2
  (`InvalidationReason` tracing, "why did this node repaint") both need
  the persistent runtime tree wired into `window.rs` first (see the
  `crates/core/src/runtime` entry above); 20.3's per-stage timings
  (reactive flush / mutation commit / layout / paint recording / scene
  upload / GPU as separate durations, not one lump `paint_duration`)
  aren't instrumented — the legacy `render_focused` path has no stage
  boundaries to time yet; 20.4 (repaint-damage/layout-invalidation/
  compositor-layer/hit-region/clip-bounds/virtualized-range visual
  overlay toggles) isn't started.
