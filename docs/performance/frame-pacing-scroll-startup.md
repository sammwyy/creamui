# Frame pacing, scrolling, incremental updates and startup

Compares `main` at `04e9679` against the same tree with:

- Animation frames requested right after a paced present (GPU surface, or
  winit on Wayland) and otherwise on a timer at the monitor's refresh rate
  instead of a fixed 16 ms; one animation timestamp per frame.
- Scroll views recorded as scroll layers in content space; the frame diff
  pairs items that entered or left the viewport and, for one layer moved by
  whole pixels over a solid backdrop, the CPU rasterizer shifts its pixels
  and repaints only the exposed strip; the GPU adds layer offsets in the
  vertex shader.
- The GPU renderer uploading only changed instances, clips and globals, and
  finding atlas slots without hashing; FxHash in the text and clip caches.
- Text layouts cached without the box height.
- `Runtime::compute_layout` syncing rects only along changed paths and
  moved subtrees; `Runtime::rebuild_hit_test` patching moved entries.
- Adapter and device requested on a background thread, a persisted driver
  pipeline cache, and a persisted, mtime-validated system font index.

## Environment

| | |
|---|---|
| Date | 2026-09-23 |
| OS | Fedora Linux 42, kernel 6.19.14-100.fc42.x86_64 |
| CPU | AMD Ryzen 7 5825U (8 cores / 16 threads) |
| GPU | AMD Radeon Graphics (Barcelo, integrated), Vulkan (RADV RENOIR) |
| Display | 1920x1080 at 60 Hz, Wayland (KDE Plasma); X11 runs use XWayland |
| Rust | rustc/cargo 1.97.1 |
| Build | `cargo build --release` |

## Methodology

The new benchmarks and examples were added first and run against both
commits from separate `git worktree` checkouts.

```sh
cargo bench -p creamui-bench --bench scroll       # adds scroll/offset_change
cargo bench -p creamui-bench --bench text_shape   # adds text_shape/resize_height
cargo bench -p creamui-bench --bench gpu
cargo bench -p creamui-bench --bench hover
cargo bench -p creamui-bench --bench runtime -- 'compute_layout|rebuild_hit_test'
cargo run -p creamui-bench --release --example animation_pacing -- 5 [--cpu]
env -u WAYLAND_DISPLAY cargo run -p creamui-bench --release --example animation_pacing -- 5 [--cpu]
cargo run -p creamui-bench --release --example gpu_uploads
CUI_DEBUG=1 cargo run -p creamui-bench --release --example startup
```

- `scroll/offset_change` scrolls a 1,000/10,000-message list in an 800x1000
  view by 40 px per frame through record, diff and the CPU rasterizer.
- `text_shape/resize_height` records one label whose box grows 1 px taller
  every frame.
- `animation_pacing` runs an animation whose output changes every frame
  for 5 s and reports the rate and the standard deviation of frame
  intervals (jitter), 3 runs each.
- `gpu_uploads` renders the scrolled 1,000-message list offscreen and
  counts bytes written to the GPU per frame.

Criterion values are medians.

## Results

### Animation pacing (60 Hz display)

| backend | before | after |
|---|---|---|
| X11, CPU | 62.2 fps, jitter 0.03–0.06 ms | **60.0 fps**, jitter 0.02–0.08 ms |
| X11, GPU | 59.7–60.3 fps, jitter 0.31–2.59 ms | 59.9 fps, jitter 0.25–0.27 ms |
| Wayland, CPU | 59.9 fps, jitter 0.57–0.66 ms | 59.9 fps, jitter 0.61–0.65 ms |
| Wayland, GPU | 59.9 fps, jitter 0.24–0.29 ms | 59.9 fps, jitter 0.20–1.14 ms |

With nothing pacing redraws (CPU on X11), the fixed 16 ms timer produced
62.2 frames per second on a 60 Hz display, so about one frame in 28 was never
shown; the timer now follows the monitor's 60.00 Hz. On paced backends the
rate already matched the display at 60 Hz. The fixed timer capped every
backend at 62.5 fps, which on 120/144 Hz displays is below the refresh
rate; the new scheduling has no such cap, but no high-refresh display was
available to measure it.

### Scrolling (CPU rasterizer)

| benchmark | before | after | change |
|---|---|---|---|
| `scroll/offset_change/1000` | 653.2 µs | 300.4 µs | **-54%** |
| `scroll/offset_change/10000` | 2.975 ms | 1.564 ms | **-47%** |
| `scroll/unchanged_frame/1000` | 121.1 µs | 129.4 µs | +7% |
| `scroll/unchanged_frame/10000` | 1.281 ms | 1.285 ms | ~0% |

A 40 px scroll step now repaints the 800x40 exposed strip and the moved
scrollbar thumb instead of the whole 800x1000 frame. `unchanged_frame`
pays for computing layer placements in the diff.

### GPU uploads per frame (`gpu_uploads`)

| frame | before | after |
|---|---|---|
| unchanged | 14,840 B | **0 B** |
| one small element changed | 14,840 B | **56 B** |
| scrolled 40 px | 14,812 B | 14,980 B |

Rows entering and leaving the viewport shift the instance list, so a
scroll step still re-uploads the instances in between (see `TODO.md`).

### Text

| benchmark | before | after | change |
|---|---|---|---|
| `text_shape/resize_height` | 16.894 µs | 0.411 µs | **-98%** (41x) |
| `text_shape/cache_hit/1000` | 236.3 ns | 175.3 ns | -26% |
| `text_shape/cache_miss` | 375.7 ns | 249.6 ns | -34% |

### Frames

| benchmark | before | after | change |
|---|---|---|---|
| `gpu/frame/dashboard` | 3.293 ms | 3.039 ms | -8% |
| `gpu/frame/grid_100x100` | 3.447 ms | 3.448 ms | ~0% |
| `hover/frame_after_hover_change/1000` | 742.6 µs | 754.1 µs | +2% |
| `hover/frame_after_hover_change/10000` | 3.958 ms | 2.844 ms | -28% |

### Runtime

| benchmark | before | after |
|---|---|---|
| `runtime/rebuild_hit_test/1000` | 12.83 µs | 42.5 ns |
| `runtime/rebuild_hit_test/10000` | 145.9 µs | 37.0 ns |
| `runtime/rebuild_hit_test/50000` | 1.661 ms | **37.5 ns** |
| `runtime/compute_layout_after_single_leaf_style_change/1000` | 293.5 µs | 299.5 µs |
| `runtime/compute_layout_after_single_leaf_style_change/10000` | 4.625 ms | 4.047 ms |
| `runtime/compute_layout_after_single_leaf_style_change/50000` | 30.92 ms | 31.25 ms |

The hit-test benchmark replaces a leaf's handlers without changing whether
it is interactive, which no longer rebuilds the list. The layout benchmark
changes one child of a single 50,000-child container, the worst case for
path-limited syncing: every direct child of a container on a changed path
is still visited (see `TODO.md`), so it is unchanged.

### Startup (`startup`, warm caches, 5–8 runs)

| step | before | after |
|---|---|---|
| Default font resolve (system font index) | 3.6–4.4 ms | 0.49–0.70 ms |
| Render pipeline creation | 5.7–6.3 ms | 0.36–0.42 ms |
| `GpuSurface::new` | 63–70 ms | 10–19 ms |
| Process start to window ready | 119–132 ms | 107–114 ms |

`GpuSurface::new` no longer includes `request_adapter` (~40 ms) and
`request_device` (~5 ms), which now run on a background thread; the first
window still waits ~50 ms for that thread, so the end-to-end gain is
~13 ms. The first run after a driver or font change rebuilds the caches
(font resolve 2.2 ms with the faster directory walk; pipeline 5.8 ms).

## Verification

- Scrolling a layer and repainting incrementally produces the same pixels
  as a full CPU render, including rounded viewports, clipped rows, an
  overlay drawn over the viewport, a gradient backdrop and fractional
  offsets (`raster::tests`).
- GPU output with a scroll layer matches the CPU rasterizer
  (`gpu::tests::scroll_layers_match_the_cpu_rasterizer`).
- `CUI_DUMP_FRAME` captures of the `showcase`, `text-editor` and `images`
  examples are identical before and after.

## Fixed along the way

Partial CPU repaints antialiased rounded corners differently from full
ones where a corner crossed the damage boundary: `tiny-skia` flattens a
curve cut by the target's edge differently from a whole one. The scratch
buffer for clipped primitives now extends past the damaged area by the
largest corner radius involved.
