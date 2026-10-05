# TODO

Current limitations and work still to do. Implemented changes belong in
`CHANGELOG.md` rather than this list.

## Persistent runtime

- Batch legacy runtime child mounting; it currently rewrites a growing child
  list for each sibling.

- Rewrite `jsx!` and `#[component]` to mount through `MountCx`, including
  reactive expressions, conditional branches, keyed lists, and events.
  The macro currently expands to widget builders. Add
  `CREAMUI_DUMP_JSX` support and validate the rewrite against the examples.
- Extend runtime composition beyond translated rectangles: add affine
  transforms, group opacity with layer promotion, rounded inherited clips,
  and transformed hit regions for those shapes. Runtime windows currently
  apply translation, per-primitive opacity, and rectangular ancestor clips.

## ABI and platforms

- Add asynchronous browser file selection and clipboard bridges. FilePicker
  is disabled on WASM unless an application hosts a local path prompt.
- Extend Android support beyond GPU presentation: add CPU presentation and
  surface recreation after suspension, and define multi-window behavior.
- Extend `WindowOptions::blur` beyond the optional KWin and blair Wayland
  protocols if X11, Windows, or macOS support is wanted.
- Verify composited per-pixel transparency for the CPU presenter on each
  platform. The code uses ARGB `wl_shm` on Wayland and passes alpha to
  `softbuffer` elsewhere; Windows and macOS behavior still needs a
  reproducible visual check.

## Rendering and performance

- Improve scrolling when a framebuffer shift is unavailable. The CPU blits
  only when one scroll layer moves by whole physical pixels over a solid,
  opaque backdrop; otherwise it repaints the layer viewport.
