# Multi-window GPU startup

Compares `main` before and after `fa8b6d5` ("perf(render): share wgpu
adapter/device across windows in one process"), which changed
`GpuSurface::new` (`crates/render/src/gpu.rs`) to request an adapter and
device only for the first GPU-backend window a process opens, reusing them
(via the new `GpuContext`) for every later window instead of repeating
`request_adapter`/`request_device` per window.

- Before: `e701ba1` (fix: better closing of disposed window handles)
- After: `fa8b6d5` (perf(render): share wgpu adapter/device across windows in one process)

## Environment

| | |
|---|---|
| Date | 2026-09-21 |
| OS | Fedora Linux 42, kernel 6.19.14-100.fc42.x86_64 |
| CPU | AMD Ryzen 7 5825U (8 cores / 16 threads) |
| GPU | AMD Radeon Graphics (Barcelo, integrated), Vulkan (RADV RENOIR) |
| Rust | rustc/cargo 1.97.1 |
| Display | Wayland (KDE Plasma) |
| Build | `cargo build --release` |

## Methodology

`crates/bench/examples/multi_window_startup.rs` opens N GPU-backend windows
in one process through `AppBuilder` and records, for each window, the time
from process start to that window's `on_window_ready` callback. It was
built and run against both commits from two isolated `git worktree`
checkouts, so only the commit under test differs.

```sh
cargo build -p creamui-bench --release --example multi_window_startup
./target/release/examples/multi_window_startup 6
CUI_DEBUG=1 ./target/release/examples/multi_window_startup 6   # per-surface timing
```

Two metrics were measured, 5 runs each:

1. **`GpuSurface::new` cost per window** (from `CUI_DEBUG=1`'s
   `"GPU surface ready on ..."` log line) — isolates exactly the function
   the change touches, unaffected by compositor/window-manager overhead.
2. **Wall-clock time to open all 6 windows** — the end-to-end effect an
   app (or a multi-window desktop-shell process) actually experiences.

## Results

### `GpuSurface::new` per window (ms), 5 runs each

Before (`e701ba1`):

| run | w0 | w1 | w2 | w3 | w4 | w5 |
|---|---|---|---|---|---|---|
| 1 | 75.80 | 51.89 | 55.44 | 61.07 | 50.05 | 59.20 |
| 2 | 78.05 | 43.30 | 45.48 | 57.32 | 46.10 | 55.89 |
| 3 | 76.86 | 49.98 | 45.75 | 59.70 | 45.88 | 58.96 |
| 4 | 79.32 | 45.40 | 45.93 | 59.60 | 45.84 | 47.36 |
| 5 | 73.80 | 50.00 | 45.51 | 55.18 | 56.01 | 53.79 |
| **median** | **76.86** | **49.98** | **45.75** | **59.60** | **46.10** | **55.89** |

After (`fa8b6d5`):

| run | w0 | w1 | w2 | w3 | w4 | w5 |
|---|---|---|---|---|---|---|
| 1 | 94.67 | 15.51 | 15.95 | 19.23 | 11.72 | 13.01 |
| 2 | 84.12 | 14.75 | 16.50 | 19.85 | 12.12 | 11.72 |
| 3 | 78.41 | 12.27 | 14.45 | 23.83 | 16.74 | 16.58 |
| 4 | 78.97 | 17.14 | 16.82 | 26.58 | 14.50 | 14.38 |
| 5 | 77.84 | 12.15 | 11.59 | 27.06 | 16.92 | 17.43 |
| **median** | **78.97** | **14.75** | **15.95** | **23.83** | **14.50** | **14.38** |

`w0` (the first GPU window, which always pays for `request_adapter` +
`request_device`) is unchanged, as expected — same cost, same code path in
both versions. `w1`-`w5` (every window after it) is where the change acts:

| | before (median) | after (median) | change |
|---|---|---|---|
| w0 (first window) | 76.86 ms | 78.97 ms | ~unchanged |
| avg of w1-w5 (each extra window) | 51.46 ms | 16.68 ms | **-67.6%** (~3.1x faster) |

### Wall-clock time to open 6 windows (ms), 5 runs each

| run | before total | after total |
|---|---|---|
| 1 | 483.87 | 255.30 |
| 2 | 456.10 | 250.95 |
| 3 | 446.86 | 275.01 |
| 4 | 492.79 | 258.98 |
| 5 | 462.33 | 264.76 |
| **median** | **462.33** | **258.98** |

**-44.0%** total time to open 6 GPU windows in one process (462.33ms →
258.98ms).

## Conclusion

The fix removes a full `request_adapter` + `request_device` round-trip
(driver/ICD work, not just a few allocations) from every GPU window after
the first one in a process. For a single-window app this change is a
no-op (only `w0` ever runs). It matters for exactly the case it targets: a
single process hosting several small GPU-backed windows — e.g. a
desktop-shell process combining panel, dock, OSD and notifications via one
`AppBuilder` with `exit_when_last_window_closes(false)` — where it cuts
the per-window GPU setup cost by roughly two-thirds.
