# TODO

Current limitations and work still to do. Implemented changes belong in
`CHANGELOG.md` rather than this list.

## Styles, text, and images

- Extend radial backgrounds beyond two-stop, farthest-corner circles to
  ellipses, explicit radii, additional color stops, and repeating gradients.

## Persistent runtime

- Wire `crates/core/src/runtime` into `Renderer` and the event handling in
  `crates/render/src/window.rs`. Windows still render the legacy
  `BoxedWidget`/`Scene` tree, so the persistent runtime cannot present frames.
- Rewrite `jsx!` and `#[component]` to mount through `MountCx`, including
  reactive expressions, conditional branches, keyed lists, and events.
  The macro currently expands to widget builders. Add
  `CREAMUI_DUMP_JSX` support and validate the rewrite against the examples.
- Verify image fit and crop when the runtime presenter consumes decoded
  image fragments. Fragments now retain the destination rect and clip ops.
- Feed `effective_transform`, `effective_opacity`, and `effective_clip` to the
  renderer. The runtime computes them, but live windows do not consume them.
  Add retained clipping and opacity composition, a layer promotion policy,
  and transforms beyond translation when that integration is built.
- Revisit wide-tree costs: layout-rect sync still visits every direct child
  of a wide container on a changed path; changes to hit-list membership or
  order rebuild the full hit list. There is no explicit z-index for absolute
  nodes.
- After runtime integration, add a tree inspector, invalidation reasons,
  and damage/layout/hit-region overlays to devtools. F3 and
  `CUI_FRAME_LOG=1` already report per-frame timings and metrics.

## Virtualization and resource loading

- Add virtual trees with flattened expanded nodes and stable keys. `TreeView`
  still materializes every visible item.
- Connect `ResourceReady::into_mutation` to the application runtime and
  presenter once runtime-backed windows are enabled.

## ABI and platforms

- Connect ABI-v2's `cui_*` runtime tree to a window and present loop. It
  currently mutates an in-memory `Runtime`; ABI-v1's `creamui_run` builds a
  separate widget tree.
- Extend `WindowOptions::blur` beyond the optional KWin and blair Wayland
  protocols if X11, Windows, or macOS support is wanted.
- Verify composited per-pixel transparency for the CPU presenter on each
  platform. The code uses ARGB `wl_shm` on Wayland and passes alpha to
  `softbuffer` elsewhere; Windows and macOS behavior still needs a
  reproducible visual check.

## Rendering and performance

- Avoid redrawing the full GPU window whenever content changes. The GPU
  clears and draws the full frame; the CPU uses calculated damage. Partial
  GPU presentation needs a way to know which previous pixels remain valid
  in each swapchain buffer.
- Improve scrolling when a framebuffer shift is unavailable. The CPU blits
  only when one scroll layer moves by whole physical pixels over a solid,
  opaque backdrop; otherwise it repaints the layer viewport.
- Evaluate mipmaps or other filtering for heavily downscaled images. GPU
  image textures have one mip level, although upload now downsizes sources
  to match their drawing size; CPU drawing uses bilinear filtering.
- Add per-glyph or batched glyph eviction if clearing the entire atlas when
  full proves costly in real workloads.
- Give the custom Wayland runtime a refresh-rate report or pacing through
  frame callbacks. It currently exposes neither, so CPU animations use the
  default 60 Hz timer.
