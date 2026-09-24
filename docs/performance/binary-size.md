# Release binary size

Compares `main` at `630b584` against the same tree with:

- `[profile.release]`: `lto = "fat"`, `codegen-units = 1`, `strip = true`.
- `wgpu` built with `default-features = false, features = ["wgsl", "dx12", "metal"]`,
  dropping the WebGPU-only `webgpu` feature (`naga/wgsl-out`).
- `env_logger` made optional behind the new `logger` feature
  (`creamui-render/logger`, forwarded as `creamui/logger` and included in
  `full`), with its default features (`regex`, `humantime`, `auto-color`)
  disabled. Without `logger`, `RUST_LOG`/`CUI_DEBUG=1` print nothing.

## Environment

| | |
|---|---|
| Date | 2026-09-23 |
| OS | Fedora Linux 42, kernel 6.19.14-100.fc42.x86_64 |
| Rust | rustc/cargo 1.97.1 |
| Target | x86_64-unknown-linux-gnu |

## Methodology

```sh
cargo build --release -p hello-world -p showcase
ls -l target/release/{hello-world,showcase}
cargo bloat --release -p hello-world --crates -n 15
```

Both versions were built from isolated `git worktree` checkouts into
separate target directories.

## Results

### File size

| binary | before | after | change |
|---|---|---|---|
| `hello-world` | 15,911,072 B (15.2 MiB) | 7,970,248 B (7.6 MiB) | **-49.9%** |
| `showcase` | 20,728,616 B (19.8 MiB) | 11,891,064 B (11.3 MiB) | **-42.6%** |

### `.text` by crate, `hello-world` (`cargo bloat --crates`)

| crate | before | after |
|---|---|---|
| naga | 1.2 MiB | 880.9 KiB |
| wgpu | 1.2 MiB | 407.8 KiB |
| std | 854.9 KiB | 1.1 MiB |
| winit | 600.9 KiB | 503.8 KiB |
| wgpu_core | 472.3 KiB | 933.3 KiB |
| wgpu_hal | 385.4 KiB | 300.1 KiB |
| creamui_render | 317.9 KiB | 182.6 KiB |
| regex_automata | 316.0 KiB | — |
| regex_syntax | 211.2 KiB | — |
| aho_corasick | 186.8 KiB | — |
| **total `.text`** | **7.6 MiB** | **5.9 MiB** |

With fat LTO, code is inlined across crates, so per-crate attribution
shifts (e.g. `wgpu` into `wgpu_core`, generics into `std`); the total is
the reliable figure.

### Resident memory, `hello-world` (GPU backend, Wayland)

| | before | after |
|---|---|---|
| VmRSS | 58,380 kB | 56,812 kB |

## Not done

- `wgpu` 22 forces the `gles` and `renderdoc` backends on Linux regardless
  of features; removing them requires upgrading `wgpu`.
- `panic = "abort"` is not used: `build_ui_with_recovery`
  (`crates/render/src/window.rs`) relies on `catch_unwind`.
- `opt-level = "s"` shrinks the binary further but was not adopted
  without a runtime benchmark.
