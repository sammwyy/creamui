# TODO

Current limitations and work still to do. Implemented changes belong in
`CHANGELOG.md` rather than this list.

## Styles, text, and images

- Extend radial backgrounds beyond two-stop, farthest-corner circles to
  ellipses, explicit radii, additional color stops, and repeating gradients.

## Persistent runtime

- Rewrite `jsx!` and `#[component]` to mount through `MountCx`, including
  reactive expressions, conditional branches, keyed lists, and events.
  The macro currently expands to widget builders. Add
  `CREAMUI_DUMP_JSX` support and validate the rewrite against the examples.
- Extend runtime composition beyond translated rectangles: add affine
  transforms, group opacity with layer promotion, rounded inherited clips,
  and transformed hit regions for those shapes. Runtime windows currently
  apply translation, per-primitive opacity, and rectangular ancestor clips.
- Revisit wide-tree costs: layout-rect sync still visits every direct child
  of a wide container on a changed path; changes to hit-list membership or
  order rebuild the full hit list. There is no explicit z-index for absolute
  nodes.
- After runtime integration, add a tree inspector, invalidation reasons,
  and damage/layout/hit-region overlays to devtools. F3 and
  `CUI_FRAME_LOG=1` already report per-frame timings and metrics.

## ABI and platforms

- Add the Android file prompt modal.
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
- Add per-glyph or batched glyph eviction if clearing the entire atlas when
  full proves costly in real workloads.
