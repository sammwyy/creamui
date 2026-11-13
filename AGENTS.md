# Agent notes

## Verification
- `cargo fmt --all` and `xvfb-run -a cargo test --workspace -- --test-threads=1` (see CONTRIBUTING.md).

## Measuring frame performance end to end
- `CUI_FRAME_LOG=1` prints per-frame timings (build/layout/record/raster/present, damage %) for apps that call `creamui_devtools::init()` (e.g. the showcase).
- Wayland blocks synthetic input, so run the app under Xvfb and drive it with `xdotool`:
  ```sh
  Xvfb :77 -screen 0 1920x1080x24 &
  env -u WAYLAND_DISPLAY DISPLAY=:77 CUI_FRAME_LOG=1 target/release/showcase > /tmp/frames.log 2>&1 &
  DISPLAY=:77 xdotool mousemove 100 400 click 5   # wheel down over the sidebar
  ```
- The default backend is CPU (`RenderBackend::Cpu`); raster cost lives in `crates/render/src/raster.rs`, damage/scroll-blit decisions in `crates/render/src/display_list.rs` (`diff`).
- Apps built with `creamui-render/platform-wayland` (e.g. `../coconut` apps) can't start under Xvfb; for measurements, temporarily build them with only `platform-winit` (the raster/diff code is the same).
