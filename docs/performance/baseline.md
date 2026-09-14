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
