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
