# Performance baseline

Numbers recorded before the incremental-runtime refactor, using
`crates/bench`. Re-run and append a new dated section after each refactor
phase lands, so a regression or an improvement has a number to point at.

## Environment

| | |
|---|---|
| Date | 2026-09-14 |
| OS | Fedora Linux 42 (KDE Plasma Desktop Edition), kernel 6.19.14-100.fc42.x86_64 |
| CPU | AMD Ryzen 7 5825U (8 cores / 16 threads) |
| GPU | AMD Radeon Graphics (Barcelo, integrated) |
| Rust | rustc/cargo 1.97.1 |
| Build | `cargo bench` (Criterion's `bench` profile: `opt-level = 3`, debug assertions off) |
| Renderer path | `creamui-render`'s `SkiaPainter` (`tiny-skia` CPU rasterizer), headless — no window or GPU surface |
| Scale factor | 1.0 |
| Criterion mode | `--quick` (fewer samples per benchmark; still statistically bounded, just not the full ~100-sample run) |

This machine has no Wayland/X11 display available (`DISPLAY` and
`WAYLAND_DISPLAY` are both unset), so only the CPU raster half of the
render path was exercised. The GPU upload/draw-call counters added in
`crates/render/src/gpu.rs` are wired up but unmeasured here — see
"Not yet measured" below.

## How to reproduce

```sh
cargo bench -p creamui-bench --bench tree_update
cargo bench -p creamui-bench --bench text_update
cargo bench -p creamui-bench --bench hover
cargo bench -p creamui-bench --bench scroll
cargo bench -p creamui-bench --bench layout
cargo bench -p creamui-bench --bench paint
```

Add `-- --quick` to any of these for a faster, lower-sample-count run (what
this baseline used).

## Results

All times are Criterion's `[low high]` confidence interval from a `--quick`
run. "Static tree" scenes use `NoopPainter` (reconcile + layout only, no
rasterization); scenes marked "raster" use the real `SkiaPainter` backend.

### `tree_update` — reconcile + layout, no raster (1920x1080 viewport)

| Scene | Nodes | Time |
|---|---|---|
| initial mount | 1,000 | 0.56 ms |
| initial mount | 10,000 | 24.3 ms |
| initial mount | 50,000 | 112.7 ms |
| unchanged rerender | 1,000 | 0.53 ms |
| unchanged rerender | 10,000 | 10.5 ms |
| unchanged rerender | 50,000 | 101.0 ms |
| deep_tree (depth 1,000), unchanged rerender | 1,000 | 9.3 ms |

`deep_tree` at depth 1,000 costs roughly 17x more than `wide_tree` at 1,000
nodes for the same node count — nested-container layout does not scale the
same way a flat list does.

### `layout` — `taffy` cost for a 100x100 grid (10,000 nodes)

| Scene | Time |
|---|---|
| initial mount | 14.5 ms |
| viewport resize (same tree) | 14.5 ms |

A resize costs about the same as the initial layout: `compute_layout` has
no way to know most of the grid's intrinsic sizing is unaffected by the
resize.

### `hover` — full repaint via `repaint_focused` after a simulated hover change (1920x1080, raster)

| Nodes | Time |
|---|---|
| 1,000 | 390 ms |
| 10,000 | 3.51 s |
| 50,000 | 3.39 s |

There is no node-local hover repaint path yet, so this is the cost of a
full-tree repaint — see REFACTOR.md 2.5. The 10,000 and 50,000 cases land
at roughly the same cost because `wide_tree`'s wrapped layout overflows the
1080px-tall viewport well before 10,000 nodes; the extra leaves beyond that
point are clipped away cheaply, so the plateau reflects viewport-visible
cost, not per-node cost.

### `scroll` — full repaint via `repaint_focused` after a simulated scroll offset change (800x1000, raster)

| Messages | Time |
|---|---|
| 1,000 | 9.57 ms |
| 10,000 | 16.6 ms |

### `text_update` — full rerender after changing one message's text (800x2,000,000 viewport, raster)

| Messages | Time |
|---|---|
| 1,000 | 103 ms |
| 10,000 | 1.06 s |

### `paint` — real CPU raster cost (raster)

| Scene | Time |
|---|---|
| `wide_tree` initial raster, 1,000 nodes (1920x1080) | 483 ms |
| `wide_tree` initial raster, 10,000 nodes (1920x1080) | 3.85 s |
| `wide_tree` initial raster, 50,000 nodes (1920x1080) | 3.76 s |
| `dashboard` initial raster (1440x900, ~120 nodes) | 27.5 ms |
| `animated_cards`, 5,000 static + 1 animated, `repaint_animated` | 0.82 ms |
| `animated_cards`, 5,000 static + 10 animated, `repaint_animated` | 0.82 ms |
| `animated_cards`, 5,000 static + 100 animated, `repaint_animated` | 1.75 ms |

`repaint_animated`'s cost tracks the number of *animated* leaves, not the
5,000 static ones sitting alongside them — the pruning check in
`scene::paint_instance` (skip a subtree with nothing promoted in it before
computing its layout) already works as intended. This is the one place in
this baseline where the current architecture already behaves the way the
target architecture should.

### Memory

`paint/initial_raster/50000` (repeated iterations of building, laying out,
and rasterizing a 50,000-leaf tree into a 1920x1080 buffer), measured with
`/usr/bin/time -v`: **303 MB** peak resident set size.

## Engine counters

`crates/core/src/metrics.rs` (behind the `perf-metrics` feature) exposes
per-frame counters; `crates/bench/tests/metrics.rs` pins their current
values so a later change shows up as a failing assertion. As of this
baseline, for a `wide_tree(100)` scene (101 nodes: 1 root + 100 leaves):

| Event | Count |
|---|---|
| Initial mount: `taffy_style_writes` / `taffy_children_writes` / `taffy_context_writes` | 101 each |
| Unchanged rerender: same three counters | 101 each (no change-detection exists yet) |
| Single-leaf paint-only change (500-node tree): `taffy_style_writes` | 501 (every node's style is rewritten, not just the changed leaf) |
| `paint_nodes_visited` per render | `2 x node_count` (the flow and absolute paint passes both walk every node, even with nothing absolutely positioned) |

## Not yet measured

- GPU upload bytes / draw calls (`crates/render/src/gpu.rs`'s
  `perf-metrics` counters) — this machine has no display server to drive a
  real `wgpu` surface. Measure on a machine with one attached and append a
  section here.
- Damage-rect count/area (`damaged_rect_count` / `damaged_pixel_area`) —
  only populated by the windowed animation-damage path in
  `crates/render/src/window.rs`, not exercised by the headless benches.

## 2026-09-14 — after Phase 1

Same environment and methodology as above. Phase 1 (`crates/core/src/scene.rs`'s
`reconcile`) now diffs a node's `taffy::Style`, measure fingerprint, and
child-id list against last frame's before writing any of them, instead of
writing unconditionally.

### Engine counters, `wide_tree(100)` (101 nodes)

| Event | Before | After |
|---|---|---|
| Unchanged rerender: `taffy_style_writes` / `taffy_context_writes` / `taffy_children_writes` | 101 each | **0 each** |
| Single-leaf paint-only change (500-node tree): `taffy_style_writes` / `taffy_context_writes` / `taffy_children_writes` | 501 each | **0 each** |

Both are now covered by `crates/bench/tests/metrics.rs` as regression
tests, not just one-off measurements.

### `tree_update` timings

| Scene | Before | After | Change |
|---|---|---|---|
| `unchanged_rerender`, 1,000 nodes | 0.53 ms | 0.36 ms | -15% |
| `unchanged_rerender`, 10,000 nodes | 10.5 ms | 6.6 ms | -26% |
| `unchanged_rerender`, 50,000 nodes | 101.0 ms | 79.0 ms | -12% |
| `deep_tree` (depth 1,000), unchanged rerender | 9.3 ms | **0.69 ms** | **-93%** |

The remaining `unchanged_rerender` cost is `reconcile` still walking the
whole ephemeral `Widget` tree every frame to *discover* nothing changed
(`reconcile_visits` is still `node_count`, unaffected by this phase) — the
work Phase 2's persistent runtime tree removes. `deep_tree`'s much larger
improvement is `taffy`'s own layout-cache invalidation: writing
`set_style`/`set_children` on every node down a 1,000-deep chain was
forcing `compute_layout` to treat the whole chain as dirty on every frame,
even when nothing in it had changed.

One regression surfaced and was fixed during this phase: caching a node's
*constrained* `taffy::Style` on `Instance` for the diff (as literally
written in REFACTOR.md 7.1's snippet) added a full extra `taffy::Style` to
every stack frame of `reconcile`'s recursion, which overflowed the stack on
`deep_tree`'s depth-1,000 chain. Comparing the *unconstrained*
`style.layout` instead (constraining only the value actually being written,
right before the write) is equivalent — `constrain_inflow` is a pure
function, so equal inputs imply equal outputs — and needs no extra stored
field at all.

### Other Phase 1 changes (not yet benchmarked as throughput numbers)

- **Hover repaint** (`crates/render/src/window.rs`): `CursorMoved` no
  longer marks the scene dirty when the hover target didn't change —
  regression-tested by
  `window::tests::cursor_moved_within_the_same_hover_region_does_not_repaint`,
  not a Criterion benchmark (the fix changes *how often* a repaint is
  triggered, not the cost of one).
- **Caret blink**: switched from the full rebuild+layout+paint path
  (`repaint`) to the paint-only path (`repaint_light`).
- **Keyed reconciliation**: `Widget::key()` lets a parent's children match
  by identity instead of position across a reorder, preserving each item's
  `taffy` node (and therefore its layer cache, focus, etc.) — regression-
  tested by `scene::tests::keyed_children_preserve_taffy_node_identity_across_a_reorder`.
  No widget in `creamui-widgets` opts into it yet.
- **`Signal::set_if_changed`**: applied to `TextController::set_cursor`/
  `set_selection`, called on every pointer-move while dragging a text
  selection.

## 2026-09-14 — after Phase 2

`crates/core/src/runtime` adds a persistent runtime tree beside the
existing `Widget`/`Instance` reconcile path (not wired into `Renderer`
yet — see REFACTOR.md Phase 3). `mount_legacy_widget` translates a
`Widget` subtree into it once; after that, a single node can be mutated
directly through a `RuntimeTransaction`, with no tree walk at all.

### `runtime` timings (`crates/bench/benches/runtime.rs`)

| Scene | Time |
|---|---|
| `mount_legacy_widget`, 1,000 nodes | 0.70 ms |
| `mount_legacy_widget`, 10,000 nodes | 55.1 ms |
| `mount_legacy_widget`, 50,000 nodes | 1.22 s |
| Direct single-leaf `SetPaintStyle`, 1,000-node tree | 19.2 ns |
| Direct single-leaf `SetPaintStyle`, 10,000-node tree | 19.2 ns |
| Direct single-leaf `SetPaintStyle`, 50,000-node tree | 19.2 ns |

The direct-mutation cost is flat across tree size — a real O(1), not just
a smaller constant — versus `tree_update/unchanged_rerender`'s 0.36 ms
(1,000 nodes) to 79 ms (50,000 nodes) for the reconcile path doing
equivalent work. That gap is the entire point of Phase 2: once something
holds a `RuntimeNodeId`, updating it no longer costs anything proportional
to the rest of the tree.

`mount_legacy_widget` itself is markedly slower than the old engine's
initial mount (1.22s vs. 133ms at 50,000 nodes) — expected for a one-time,
not-yet-optimized translation pass (arena allocation plus per-field
mutation dispatch for every node), and irrelevant to the O(1) result
above as long as mounting stays a one-time cost. Revisit if Phase 3 ends
up calling it more than once per mount.

### Correctness

`runtime::tests::random_create_insert_remove_sequences_never_break_invariants`
(500 pseudo-random create/insert/remove operations against
`Runtime::check_invariants`) caught a real bug during development: reusing
`insert_child` to move an already-attached child produced a duplicate
entry in its parent's `Children` list. Fixed by making `Children::insert`
idempotent (remove-then-reinsert) rather than append-only.

## 2026-09-14 — after Phase 3

`creamui-reactive` adds `Owner` (a disposable scope for effects and child
scopes). `crates/core/src/runtime` adds `create_binding` (ties a
`Signal::get()` read directly to a `RuntimeTransaction` mutation, no
widget rebuild involved), `create_branch` (conditional mount/unmount
anchored at a stable parent, the runtime analogue of
`if condition.get() { <Panel/> }`), and `create_keyed_list` (an
order-preserving keyed list: a surviving key keeps its runtime node,
only new/removed keys mount or dispose).

### `runtime` timings (`crates/bench/benches/runtime.rs`)

| Update path | 1,000 nodes | 10,000 nodes | 50,000 nodes |
|---|---|---|---|
| Reconcile (`tree_update/unchanged_rerender`, Phase 1) | 0.36 ms | 6.6 ms | 79.0 ms |
| Direct `RuntimeTransaction` mutation (Phase 2) | 19.2 ns | 19.2 ns | 19.2 ns |
| `Signal::set` through `create_binding` (Phase 3) | 74.2 ns | 77.0 ns | 74.9 ns |

The reactive-driven path is ~75ns flat regardless of tree size — a few
nanoseconds more than a raw transaction (the effect dispatch overhead),
still completely independent of how many other nodes exist. That's Phase
3's exit criterion ("benchmarks show update cost no longer scales with
unrelated tree size") holding for the actual signal-to-mutation path, not
just the lower-level transaction API underneath it.

### Correctness

- `create_branch`: toggling a `Signal<bool>` mounts/unmounts a subtree;
  hiding disposes bindings created inside it; disposing the enclosing
  `Owner` stops the branch from reacting to further toggles.
- `create_keyed_list`: reordering a `Signal<Vec<T>>` preserves each
  surviving key's `RuntimeNodeId` (not just its data); a removed key
  disposes its own subtree and any bindings created for it.
- `Owner::dispose` is idempotent, cascades to child scopes, and (via
  `RefCell`-free `Rc` ownership with no parent back-pointer) cannot leak
  through a reference cycle.

### Known limitations (see `TODO.md`)

- `Owner` has no parent back-pointer, so a disposed child scope is not
  removed from its parent's `children` list — only reclaimed once the
  parent itself is disposed. For a value toggled/reordered many times
  under one long-lived parent (e.g. a checkbox flipped thousands of
  times), this accumulates small dead `Owner` entries until the parent
  goes away. Not a subscription leak (effects/cleanups are correctly
  disposed), just deferred memory reclamation.
- Nothing in `window.rs`/`Renderer` calls `create_binding`/`create_branch`/
  `create_keyed_list` yet — REFACTOR.md's Phase 4 (JSX/component
  compilation) is what should start generating these calls from
  application code instead of a widget-rebuilding closure.

## 2026-09-14 — Phase 4 (partial: mount/bind foundation only)

`crates/core/src/runtime` adds `MountCx` (`container`/`text`/`image`/
`bind`/`branch`/`keyed`/`on_cleanup`, matching REFACTOR.md 10.2's sketch)
and `View`/`IntoView` (REFACTOR.md 10.5 — a component can return an
already-mounted `RuntimeNodeId` or a legacy `BoxedWidget`, auto-adapted
through `mount_legacy_widget`). Both are plain Rust APIs built on Phase
3's `create_binding`/`create_branch`/`create_keyed_list`, exercised
end-to-end by tests (`mount_cx.rs`, `view.rs`) including a component that
calls `cx.container()` inside a `cx.branch()`/`cx.keyed()` callback — the
realistic nested-mounting shape a real component tree would produce.

**Not done**: the actual `jsx!` proc-macro rewrite (REFACTOR.md 10.1,
10.3, 10.4, 10.6, 10.7) — the macro in `crates/macros` still expands to
widget-builder calls, unchanged. Rewriting a 1,350-line proc macro that
every example and `creamui-jsx`/`creamui-dynamic` depend on, to instead
categorize each JSX expression (static/reactive/event/conditional/keyed)
and emit `MountCx` calls, is a large, high-blast-radius change that needs
its own dedicated pass with the full example suite as a correctness
check — attempting it within this pass risked shipping something
half-verified. `MountCx`/`View` are exactly what that rewrite would need
to target, built and tested first so that work has a stable foundation.

### Bug found while building this

A branch/keyed `mount`/`render` closure that itself opened a nested
mount (e.g. `cx.container()` called from inside a `cx.branch()` callback)
used to panic: `create_branch`/`create_keyed_list` invoked the closure
from inside an already-open `RuntimeTransaction`, and `MountCx`'s own
calls each open their own transaction via `SharedRuntime`, producing a
`RefCell` double-borrow. Fixed by changing the `mount`/`render` closure
signature to receive `&SharedRuntime` instead of `&mut RuntimeTransaction`,
so nested mounting manages its own transactions instead of reusing one
held open by the caller. Regression-tested
(`branch::tests::a_mount_closure_that_recurses_into_another_mount_does_not_panic`).

## 2026-09-14 — Phase 5

`Runtime` now owns a real `taffy::TaffyTree`. `RuntimeTransaction` mutates
it precisely: `SetLayoutStyle` diffs and writes `taffy`'s style only on
change, `SetMeasure` skips the `taffy` context write when the fingerprint
is unchanged, `SetPaintStyle`/`SetTypographyStyle`/`SetText`/`SetTransform`
never touch `taffy` at all. `create_node`/`insert_child`/`reorder_children`/
`remove_subtree` keep a parallel `taffy` node per runtime node in sync,
checked by `check_invariants` (taffy children must match runtime children
for every node). `Runtime::compute_layout` skips `taffy::compute_layout`
entirely unless something marked layout dirty or the viewport itself
changed, then marks `PAINT | HIT_TEST` on nodes whose window-space rect
actually moved and returns their old+new rects as damage.

### Measured

`runtime/compute_layout_after_single_leaf_style_change`
(`crates/bench/benches/runtime.rs`): changing one leaf's layout style in a
`wide_tree`, then calling `compute_layout`:

| Nodes | Time |
|---|---|
| 1,000 | 0.16 ms |
| 10,000 | 2.2 ms |
| 50,000 | 23.5 ms |

This scales close to linearly with tree size — **not** flat, unlike the
paint-mutation benchmarks in the Phase 2/3 sections above. The cost is not
`taffy::compute_layout` itself (internally cached, so recomputing one
leaf's size shouldn't touch unrelated subtrees) but
`Runtime::sync_layout_rects`, the post-layout pass that walks every node
to detect which rects moved. It's an O(n) scan regardless of how many
nodes `taffy` actually recomputed, since nothing in `taffy`'s public API
used here exposes which nodes it touched. REFACTOR.md 11.8's "layout
metrics scale with affected branches rather than whole-tree mutation
count" is therefore only partially met — true for `taffy` writes and
`compute_layout` invocation, not for this rect-sync pass. See `TODO.md`.

Paint-only changes never reaching `taffy` at all is fully met and
unit-tested (`paint_only_changes_never_mark_layout_dirty`).

## 2026-09-14 — Phase 6

`Runtime` gains node-ID-based interaction state: `EventState` (retained
per node, replacing on-click/hover/drag/scroll/focus/cursor handlers set
wholesale via `Mutation::SetEventHandlers` rather than extracted from a
widget during paint), a retained `hit_entries`/`focus_order` list rebuilt
by `rebuild_hit_test` only when `HIT_TEST`/`STRUCTURE` dirty, and
node-ID `hovered`/`pressed`/`focused`/`pointer_capture` state.
`set_hovered`/`set_pressed`/`set_focused` return `false` (marking nothing)
when the target is unchanged. `scroll_target` walks the parent chain from
a hit point to the nearest node with `on_scroll`. `next_focus` cycles the
retained focus order by node ID, not index.

### Measured

`runtime/rebuild_hit_test` (`crates/bench/benches/runtime.rs`), one
interactive leaf in an otherwise plain `wide_tree`:

| Nodes | Time |
|---|---|
| 1,000 | 4.4 µs |
| 10,000 | 53.1 µs |
| 50,000 | 787 µs |

Scales with total tree size, not interactive-node count — the same
whole-tree-walk limitation as Phase 5's `sync_layout_rects`, and for the
same reason: finding which nodes are interactive requires visiting every
node once. "Mouse movement with no hover transition performs zero
painting" (12.9) is unaffected by this — `set_hovered` on an unchanged
target is a plain field comparison, independent of tree size, and doesn't
call `rebuild_hit_test` at all.

### Not done

Z-order/stacking-context handling (12.2): hit entries are collected in
plain depth-first child order, with no equivalent of the legacy
`Scene`'s Flow/Absolute two-pass distinction for absolutely positioned
nodes. A spatial index (12.4, tile buckets or similar) is also not
attempted — REFACTOR.md 12.4 itself says to benchmark before adding one,
and nothing here has shown `rebuild_hit_test`'s linear scan to be the
actual bottleneck yet.

## 2026-09-14 — Phase 7

`crates/core/src/runtime/paint.rs` adds `PaintPrimitive`/`PaintOp`
(Quad/Border/Text/Image, plus push/pop clip and transform ops) and a
per-node `PaintFragment` (`ops` + `bounds`), regenerated by
`Runtime::rebuild_paint`. Unlike `compute_layout`/`rebuild_hit_test`
(Phases 5/6, both full-tree walks gated by a single dirty flag), paint
regeneration is targeted: every mutation that marks a node PAINT-dirty
appends it to a `paint_queue`, and `rebuild_paint` only visits that
queue — genuinely O(affected nodes), not O(tree size). `RecordingPainter`
implements the existing `Painter` trait to capture a legacy
`Widget::paint` call as `PaintOp`s instead of rasterizing it (REFACTOR.md
13.2's migration path), verified against a real `Widget` impl.

### Measured

`runtime/rebuild_paint_after_single_leaf_paint_change`
(`crates/bench/benches/runtime.rs`), changing one leaf's background in a
`wide_tree` then calling `rebuild_paint`:

| Nodes | Time |
|---|---|
| 1,000 | 67.8 ns |
| 10,000 | 67.3 ns |
| 50,000 | 66.8 ns |

Flat, unlike Phase 5's `compute_layout` (0.16ms → 23.5ms) and Phase 6's
`rebuild_hit_test` (4.4µs → 787µs) for the equivalent single-leaf-change
shape — because rebuilding paint order (the one genuinely O(tree-size)
part of this pass) is gated on STRUCTURE changing, and a paint-only
mutation never touches structure.

### Not done

- `NodeKind::Custom` nodes (legacy-mounted widgets) get an empty
  fragment — `generate_fragment` only knows how to produce primitives for
  native `Container`/`Text`/`Image` nodes, since the widget that could
  paint a `Custom` node is dropped after `mount_legacy_widget` runs (see
  Phase 2's baseline entry). Giving legacy-mounted subtrees real retained
  paint output would need keeping their widgets alive post-mount,
  contradicting that design.
- Damage merging/collapse-to-full-window (REFACTOR.md 13.5) is not
  implemented — `rebuild_paint` returns raw old+new bounds per
  regenerated fragment with no overlap merging or area-threshold
  collapse. REFACTOR.md itself frames this as "tune with benchmarks",
  not a Phase 7 requirement.
- Subtree effects (13.4 — opacity groups, transforms, shadows, filters,
  scroll transforms) have no retained representation beyond the
  `PushTransform`/`PopTransform` op variants existing in the enum;
  nothing generates them yet. `RuntimeNode` has no clip/opacity-group
  flag, so clipping containers don't emit `PushClip`/`PopClip` either.

## 2026-09-14 — Phase 8

`crates/render/src/gpu_scene/` adds an instanced-quad `wgpu` pipeline
(REFACTOR.md 14.2-14.5) alongside the existing CPU-raster-then-blit path
in `crates/render/src/gpu.rs`, which is untouched. `QuadStore` is a
free-list-backed store of `QuadInstance`s addressed by stable
`GpuPrimitiveId`s, tracking the smallest contiguous index range touched
since the last flush; `GpuSceneState::sync_node` diffs a `RuntimeNodeId`'s
new `PaintFragment` against its previously-allocated ids, reusing slots
in place rather than reallocating, so `render()` uploads only that dirty
range instead of the whole instance buffer (14.5). One instanced
`draw_call` covers every live quad (14.4) — draw calls scale with
batches, not with widget count. `quad_instances_for_fragment` translates
`PaintPrimitive::Quad`/`Border` into instances; `Text`/`Image` and
`PushClip`/`PushTransform` are not yet consumed, matching REFACTOR.md
14.8's migration order (solid quad and border land before images, text,
and shadows).

### Measured

`gpu_scene/single_update` (`crates/bench/benches/gpu_scene.rs`), updating
one already-inserted quad in a `QuadStore` of the given size and reading
back the dirty range:

| Quads in store | Time |
|---|---|
| 1,000 | 23.1 ns |
| 10,000 | 22.6 ns |
| 50,000 | 22.6 ns |

Flat, same shape as Phase 7's `rebuild_paint` result — a single quad
update costs the same regardless of how many other quads share the
store, because the dirty range is one slot wide either way.

`gpu_scene/insert_many`, populating an empty store from scratch:

| Quads | Time |
|---|---|
| 1,000 | 23.7 µs |
| 10,000 | 235.2 µs |
| 50,000 | 1.166 ms |

Linear in quad count, as expected for populating every slot once — the
benchmark this phase's O(1) claim applies to is the single-quad update
above, not a from-scratch mount.

### Not yet measured

This machine has no display server (see "Environment" above), so nothing
past `QuadStore` and the pure `PaintFragment` -> `QuadInstance`
translation could be exercised — `GpuSceneState` itself (pipeline
creation, shader compilation, buffer upload/growth, the actual draw call)
has never run against a real `wgpu` surface. Needs a windowed smoke test
on a machine with a GPU and display attached before the pipeline itself
is trusted, same caveat as `crates/render/src/gpu.rs`'s counters.

### Not done

- Not wired into `Renderer`/`window.rs` — no `RenderBackend::GpuScene`
  variant exists yet, and wiring it in needs `crates/core/src/runtime`
  wired in first, since `sync_node` consumes that module's
  `RuntimeNodeId`/`PaintFragment` types.
- Text, images, clips, and transforms are not rasterized by this
  pipeline (REFACTOR.md 14.8's later migration steps).
- The instance buffer only grows, never shrinks back down after a scene
  loses most of its quads.
- Images/texture manager (14.6) and clip strategy (14.7) are not
  started; Vello is not evaluated as an alternative backend (14.9).

## 2026-09-14 — Phase 9

`crates/render/src/gpu_scene/text.rs` adds `ShapeCache` (REFACTOR.md
15.2/15.5: an LRU from `(text, font size, wrap width, family, weight)` to
a `ShapedRun` shaped once at a canonical `(0, 0)`/left-aligned origin,
shared by measurement-shaped and paint-shaped uses of the same text) and
`GlyphAtlas` (15.4: a shelf-packed region tracker for rasterized glyph
bitmaps, `GpuSceneState` owns the matching `R8Unorm` GPU texture and
uploads a glyph's bitmap once the first time it's placed). `GpuPrimitiveId`
-> `GlyphPrimitiveId`/`GlyphStore` mirrors Phase 8's `QuadStore` free-list-
plus-dirty-range shape for glyph instances. `GpuSceneState::sync_text_node`
ties these together: shape (cache hit on unchanged text), place every
glyph in the atlas (rasterizing only on a miss), and diff the resulting
`GlyphInstance`s against the node's previous ones the same incremental
way `sync_node` already did for quads. `TextPrimitive` (`crates/core`)
gained `family`/`bold` fields, populated by `generate_fragment` from
`RuntimeNode::typography_style`, since shaping needs to know which face
to resolve — Phase 7 hadn't carried that through.

The first version of `ShapeCache`'s LRU used a `VecDeque` reordered on
every hit (an O(cache size) linear scan + shift per lookup) — caught
before committing by writing the flatness benchmark below and noticing it
wouldn't actually be flat. Replaced with a logical clock: a hit is one
hashmap lookup plus a counter bump (`O(1)`), and only eviction (a miss
past capacity) scans for the least-recently-used entry.

### Measured

`text_shape/cache_hit` (`crates/bench/benches/text_shape.rs`), re-shaping
one already-cached string with the given number of other distinct
strings also warm in the cache:

| Other cached strings | Time |
|---|---|
| 10 | 56.7 ns |
| 1,000 | 56.9 ns |
| 10,000 | 64.2 ns |

`text_shape/cache_miss`, shaping a new never-seen string each iteration
(a fresh `fontdue` layout pass): **4.62 µs** — about 80-100x a cache hit,
confirming the cache is actually doing the expensive part once.

### Not yet measured

Same "no display server" constraint as Phases 0 and 8:
`GpuSceneState::sync_text_node`'s atlas texture upload and the glyph
pipeline's draw call have never run against a real `wgpu` surface.

### Not done

- `sync_text_node` is reachable only through the still-unwired
  `gpu_scene` path — the live legacy renderer (`text_metrics.rs`,
  `render/font.rs`) is untouched and still reshapes on every measurement
  and every paint. See `TODO.md`.
- `GlyphAtlas::grow` exists and is unit-tested but `GpuSceneState` never
  calls it — the atlas is a fixed 1024x1024 texture, and a glyph that
  doesn't fit is silently dropped rather than triggering a resize, since
  a real resize also needs every already-placed `GlyphInstance`'s UV
  rebaked (see `TODO.md` for why that wasn't attempted here).
- No selection highlighting, underline/strikethrough, or per-glyph color
  runs in the GPU text path.
- Image primitives, clips, and transforms remain unrasterized by
  `gpu_scene`, same as the Phase 8 baseline entry.

## 2026-09-14 — Phase 10

`Runtime::rebuild_composite` (REFACTOR.md 16.1) gives `Transform2D` a
real property-only update path: `RuntimeNode`/`Mutation::SetTransform`
already existed but nothing consumed `DirtyFlags::COMPOSITE` or read
`transform` back — `generate_fragment` never referenced it. A
`composite_queue` (mirroring `paint_queue`) now drives a pass that
computes each dirty node's `effective_transform` (its own `transform`
plus its parent's `effective_transform`) and cascades to children only
when a node's own value actually changed, since they inherit it — no
layout, no paint-fragment regeneration, matching the exit criterion
"Transform-only animations avoid paint-record regeneration" directly
(a dedicated test asserts a transform change leaves the paint fragment's
bounds untouched). `crates/render/src/gpu_scene` gained the consumer
side: `sync_node`/`sync_text_node` now take a `Transform2D` and bake it
into instance positions, and a new `GpuSceneState::sync_transform`
repositions a node's already-retained quad/glyph instances by the delta
from its previously-applied transform — no `ShapeCache`, atlas, or
`PaintFragment` involved at all.

### Measured

`runtime/rebuild_composite_after_single_leaf_transform_change`
(`crates/bench/benches/runtime.rs`), changing one leaf's transform in a
`wide_tree` then calling `rebuild_composite`:

| Nodes | Time |
|---|---|
| 1,000 | 59.2 ns |
| 10,000 | 57.3 ns |
| 50,000 | 57.4 ns |

Flat, the same shape as Phase 7's `rebuild_paint` and Phase 9's
`ShapeCache` hit numbers — a leaf has no children to cascade to, so its
transform change costs exactly one node's worth of work regardless of
how large the surrounding tree is.

### Not yet measured

`GpuSceneState::sync_transform`'s actual GPU buffer patch — same "no
display server" constraint as every `gpu_scene` entry above.

### Not done

- `rebuild_composite` and `sync_transform` are tested/benchmarked in
  isolation but nothing calls either from a shared place yet — see
  `TODO.md`.
- Only translation (`Transform2D { x, y }`) — no scale/rotate, and no
  `OpacityNode`/`ClipNode` (16.1's other two property kinds).
- No compositor-owned layer-promotion policy or devtools layer-memory
  view (16.4/16.5) — the legacy `scene.rs` reconcile path's own
  paint-time layer-promotion heuristic is untouched.
- Scrolling is not transform-first (16.2) — no scroll view routes its
  offset through `Mutation::SetTransform` yet.

## 2026-09-14 — Phase 11

`creamui_core::HeightIndex` is a Fenwick tree over per-item heights:
`set_height`/`push` are `O(log n)` point updates, `offset`/`total_height`
are `O(log n)` prefix-sum reads, and `index_at_offset` is the `O(log n)`
binary-search "find by prefix sum" that turns a scroll offset into an
item index (REFACTOR.md 17.2). `visible_range` layers overscan and
bounds-clamping on top to get the index range a virtualized list should
mount. `truncate` rebuilds in `O(n)`; there is no mid-sequence
insert/remove yet (see `TODO.md`).

### Measured

`virtual_list/set_height` and `virtual_list/index_at_offset`
(`crates/bench/benches/virtual_list.rs`), on a `HeightIndex` of the
given size:

| Items | `set_height` | `index_at_offset` | `visible_range` |
|---|---|---|---|
| 1,000 | 3.25 ns | 13.2 ns | 16.4 ns |
| 10,000 | 2.90 ns | 15.4 ns | 20.8 ns |
| 100,000 | 3.47 ns | 15.3 ns | 27.8 ns |

Logarithmic, not flat: a 100x increase in item count costs roughly a
constant few nanoseconds more, not a 100x slowdown — the `O(log n)`
shape 17.2 asks for, distinct from the `O(1)` shape of Phases 7/9/10's
single-node update benchmarks.

### Not done

- No `VirtualList` widget and no caller anywhere — `HeightIndex`/
  `visible_range` are unused outside their own tests and benchmark.
- No recycling pool (17.3), virtual table (17.4), or virtual tree (17.5).
