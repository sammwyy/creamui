# Memory footprint: fonts, shared GPU resources and images

Compares `main` at `1b23a26` against the same tree with:

- `fontdue` replaced by `swash`: font files are memory-mapped and parsed
  on demand, and `creamui_fonts::layout` shapes and wraps text for both
  measurement and painting.
- One `GpuShared` per device holding the shader, pipelines, glyph atlas and
  image textures for every GPU window, and one `TextSystem` per UI thread.
- `Instance` packed from 128 to 56 bytes (RGBA8 colors, clips moved to a
  per-frame lookup texture indexed from the vertex shader).
- A full glyph atlas cleared of stale glyphs before growing, shrunk when
  mostly unused; the instance buffer shrinks after large frames.
- Images uploaded at the smallest power-of-two reduction still at least as
  large as they are drawn, and decoded pixels of `ImageData` loaded from
  bytes or a path dropped after upload (decoded again on demand).

## Environment

| | |
|---|---|
| Date | 2026-09-23 |
| OS | Fedora Linux 42, kernel 6.19.14-100.fc42.x86_64 |
| CPU | AMD Ryzen 7 5825U (8 cores / 16 threads) |
| GPU | AMD Radeon Graphics (Barcelo, integrated), Vulkan (RADV RENOIR) |
| Rust | rustc/cargo 1.97.1 |
| Display | Wayland (KDE Plasma) |
| Build | `cargo build --release` |

## Methodology

`crates/bench/examples/memory_footprint.rs` opens N 640x360 windows, each
showing one line of mixed Latin/CJK/Hangul text and optionally a
5120x2880 JPEG (3.9 MB) drawn at 240x135, waits 2 s after the last one is
ready, and prints `VmRSS`/`RssAnon` from `/proc/self/status` and the
`drm-memory-vram`/`drm-memory-gtt` counters from `/proc/self/fdinfo`.
The same example source was built against both commits.

```sh
IMG=/usr/share/wallpapers/Flow/contents/images/5120x2880.jpg
cargo run -p creamui-bench --release --example memory_footprint
cargo run -p creamui-bench --release --example memory_footprint -- --windows 6
cargo run -p creamui-bench --release --example memory_footprint -- --font NotoSansCJK
cargo run -p creamui-bench --release --example memory_footprint -- --image $IMG
cargo run -p creamui-bench --release --example memory_footprint -- --windows 6 --image $IMG
cargo run -p creamui-bench --release --example memory_footprint -- --cpu
cargo run -p creamui-bench --release --example memory_footprint -- --cpu --image $IMG
```

The default font resolved to Liberation Sans; `--font NotoSansCJK`
selects `NotoSansCJK-VF.ttc` (65,535 glyphs). Each scenario ran 3 times;
the tables show the median, in kB.

## Results

### GPU backend

| scenario | RSS before | RSS after | anon before | anon after | VRAM before | VRAM after | GTT before | GTT after |
|---|---|---|---|---|---|---|---|---|
| 1 window | 54,564 | 51,792 | 16,872 | 9,944 | 3,380 | 3,388 | 21,320 | 21,320 |
| 6 windows | 55,660 | 51,924 | 17,796 | 10,340 | 18,776 | 18,784 | 22,616 | 22,616 |
| 1 window, CJK font | 395,624 | 54,296 | 357,856 | 10,624 | 3,380 | 3,388 | 21,320 | 21,320 |
| 1 window, image | 112,388 | 73,140 | 74,512 | 31,336 | 3,388 | 3,388 | 137,800 | 21,320 |
| 6 windows, image | 113,596 | 73,316 | 75,508 | 31,464 | 18,068 | 18,784 | 434,520 | 22,616 |

- **CJK font: -86% RSS** (395.6 MB → 54.3 MB). `fontdue` decoded every
  outline of the 65k-glyph face up front; the mapped file now only
  contributes the pages actually read.
- **Base footprint: -41% anonymous memory** (16.9 MB → 9.9 MB) from the
  same effect on the default Latin face.
- **Image: -116 MB GTT** for one window and **-412 MB GTT** for six
  (434.5 MB → 22.6 MB): each window used to upload its own full-resolution
  59 MB texture; now one 320x180 texture is shared.
- Of the ~21 MB of anonymous memory the image rows add over the text-only
  rows, 3.9 MB are the retained encoded bytes and the rest is freed decode
  scratch kept by glibc's dynamic mmap threshold: with
  `MALLOC_MMAP_THRESHOLD_=131072` the same run reports 13,624 kB.
- VRAM is dominated by the swapchains and is unchanged within 1 MB.

### CPU backend

| scenario | RSS before | RSS after | anon before | anon after |
|---|---|---|---|---|
| 1 window | 15,404 | 11,096 | 9,084 | 2,212 |
| 1 window, image | 73,792 | 75,424 | 67,128 | 66,020 |

- **-76% anonymous memory** without images (9.1 MB → 2.2 MB).
- With the image, the saving on fonts is offset by the 3.9 MB of encoded
  bytes `ImageData` now keeps; the CPU backend never drops decoded pixels
  (see the current measurement below).

### CPU image retention decision (2026-09-28)

Rebuilt `memory_footprint` in release mode on the current tree and ran each
CPU scenario three times under Xvfb using the same
`/usr/share/wallpapers/Flow/contents/images/5120x2880.jpg`. The table shows
median `RssAnon` from `/proc/self/status`, in KiB:

| CPU windows | Without image | With image | Difference |
|---|---:|---:|---:|
| 1 | 2,356 | 66,048 | 63,692 |
| 6 | 7,056 | 69,404 | 62,348 |

The JPEG is 3,907,925 bytes (3,816 KiB); its decoded 5120×2880 RGBA
pixels are 58,982,400 bytes (57,600 KiB). The retained encoded copy adds
6.6% over the decoded pixel size per unique image. Six windows share the
same `ImageData`, so the image memory does not grow sixfold. Process memory
also includes decoder and allocator effects, so the table's differences
are not an isolated measurement of the encoded allocation.

Keep the encoded bytes for now. `ImageData` can be shared by CPU and GPU
windows; the GPU upload path discards decoded pixels and needs the bytes to
reload them. The measured 3.8 MiB per unique image does not justify a
backend-specific image representation or changing that sharing contract.

### CPU cost

`cargo bench -p creamui-bench --bench text_shape` and `--bench gpu`,
criterion medians:

| benchmark | before | after |
|---|---|---|
| `text_shape/cache_miss` | 2.334 µs | 0.327 µs |
| `text_shape/cache_hit/1000` | 224.9 ns | 235.1 ns |
| `gpu/frame/dashboard` | 3.390 ms | 3.436 ms |
| `gpu/frame/grid_100x100` | 3.791 ms | 3.686 ms |

A text layout cache miss is 7.1x faster with `swash`. Cache hits and GPU
frame times are within a few percent.

`multi_window_startup 6` (5 runs) was dominated by compositor variance:
median total 270.3 ms before, 242.9 ms after.

## Rendering changes

Glyph advances are no longer rounded up to whole pixels and kerning is
applied, so text is slightly narrower and more evenly spaced than with
`fontdue`. Bold text always uses the system's bold face; before, it fell
back to the regular one whenever the regular weight had been loaded first.

## CPU-only builds and UPX (2026-10-30)

Measured the workspace's `hello-world` example (two labels and a button),
480×320 physical pixels, on Fedora 42, kernel 6.19.14, Rust 1.97.1 and
Wayland. Both builds use the CPU renderer through the default winit host.
The baseline is commit `006dd85`; the comparison disables the new `gpu`
feature, leaving `platform-winit` enabled. Release settings are unchanged.

Each row is the median of three sequential runs: wait three seconds after
launch, sample process CPU counters for three seconds without input, then
read `/proc/PID/smaps_rollup`. UPX 5.1.1 used its default compression settings
on separate copies of the release executables. Memory columns are KiB;
file sizes are bytes.

| Build | File size | RSS | PSS | Anonymous |
|---|---:|---:|---:|---:|
| Baseline, runtime CPU selection | 9,729,256 | 11,036 | 7,515 | 1,904 |
| Baseline + UPX | 4,020,448 | 11,916 | 8,498 | 3,704 |
| CPU-only compilation | 5,321,872 | 10,024 | 6,555 | 1,812 |
| CPU-only compilation + UPX | 2,194,628 | 10,440 | 7,039 | 2,772 |

Excluding GPU support reduces the executable by 45.3% and RSS by 9.2%
(about 1 MiB) without changing the window's pixels. `wgpu`, `wgpu-core`,
`wgpu-hal`, `naga` and `ash` are absent from the CPU-only dependency tree.
The runtime CPU selection already skipped GPU initialization; most of the
improvement comes from the smaller executable rather than freed GPU state.

There were no additional process CPU ticks or main-thread voluntary context
switches in any of the twelve idle sampling windows. This is a finite idle
sample, not a long-running leak test. The original and CPU-only builds,
including the latter's default backend with no environment override,
produced byte-identical PNG frame captures.

UPX reduced disk size but increased RSS and private dirty memory in these
runs. Disk compression is not a RAM optimization. UPX's
[Linux executable documentation](https://github.com/upx/upx/blob/devel/doc/upx.pod#notes-for-linuxelf386)
describes decompression into RAM and the lifetime of the decompressed pages.
RSS includes executable code, mapped fonts, shared libraries and SHM pixels;
it is not a measure of the widget heap. PSS apportions shared pages and varies
with what other processes are running. A 30 MiB minimum was not reproduced
with this example and window configuration.

To repeat the comparison, preserve the first executable before rebuilding:

```sh
cargo build -p hello-world --release
cp target/release/hello-world target/hello-default
cargo build -p hello-world --release --no-default-features --features platform-winit
cp target/release/hello-world target/hello-cpu
upx -o target/hello-default-upx target/hello-default
upx -o target/hello-cpu-upx target/hello-cpu
python3 crates/bench/scripts/idle_memory.py target/hello-default
python3 crates/bench/scripts/idle_memory.py target/hello-default-upx
python3 crates/bench/scripts/idle_memory.py target/hello-cpu
python3 crates/bench/scripts/idle_memory.py target/hello-cpu-upx
```

The profiler requires Linux `/proc` and a display session. It terminates each
application after sampling. Do not interact with the window during a run.
The script forces `CUI_OVERRIDE_RENDER_BACKEND=cpu` unless `--backend gpu` is
specified. `--seconds` and `--settle` adjust the sample and settling durations.
Cargo features are additive: a different dependency enabling `gpu` includes
GPU support again, even if the application's direct dependency disables it.

A separate single run with CPU-only `platform-wayland` produced 9,080 KiB
RSS and a 3,988,224-byte executable. It also registered about 124 voluntary
context switches per second and 0.33% process CPU: the native event loop
polls for user events every 8 ms. This differs from winit's blocking idle
behavior and remains tracked in `TODO.md`. The CPU raster scratch vector
and Wayland SHM mappings also retain their high-water capacities after a
large window shrinks; reclaiming them would target post-resize idle memory,
not this initial-window baseline. A full 480×320 RGBA buffer is 600 KiB;
doubling the display scale quadruples each buffer's pixel storage.
