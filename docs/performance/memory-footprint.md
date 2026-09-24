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
  (see `TODO.md`).

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
