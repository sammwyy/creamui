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
  `create_branch`, and `create_keyed_list`. REFACTOR.md Phase 4 (JSX/
  component compilation) is what should start generating calls into these
  from application code instead of a widget-rebuilding closure.
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
