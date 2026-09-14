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
