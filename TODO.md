# TODO

Current limitations and work still to do. Implemented changes belong in
`CHANGELOG.md` rather than this list.

## Window chrome

- Exercise the Win32 `AppTitlebar` frame on Windows 10 and 11 hardware: caption
  removal, maximized insets, top-edge resizing and the snap layouts flyout are
  only covered by cross-compilation so far.

## Persistent runtime

- Rewrite `jsx!` and `#[component]` to mount through `MountCx`, including
  reactive expressions, conditional branches, keyed lists, and events.
  The macro currently expands to widget builders. Preserve control events
  and hooks in retained composition and validate the rewrite against the
  examples.
- Extend runtime composition beyond translated rectangles: add affine
  transforms, group opacity with layer promotion, rounded inherited clips,
  and transformed hit regions for those shapes. Runtime windows currently
  apply translation, per-primitive opacity, and rectangular ancestor clips.

## ABI and platforms

- Extend `WindowOptions::blur` beyond the optional KWin and blair Wayland
  protocols if X11, Windows, or macOS support is wanted.
- Verify composited per-pixel transparency for the CPU presenter on each
  platform. Wayland uses ARGB `wl_shm`, Android uses native RGBA buffers,
  and other targets pass alpha to `softbuffer`. Windows and macOS behavior
  still needs a reproducible visual check.

## Rendering and performance

- Replace the native Wayland event loop's 8 ms user-event polling with a
  wake descriptor for proxy events and queued window requests, preserving
  redraw and timer delivery while allowing an idle app to block.
- Reclaim oversized CPU scratch and released Wayland SHM buffers after a
  window shrinks; both currently retain their high-water allocation.
- Improve scrolling when a framebuffer shift is unavailable. The CPU blits
  only when one scroll layer moves by whole physical pixels over a solid,
  opaque backdrop; otherwise it repaints the layer viewport.

## Tooling

- Clear existing strict Clippy diagnostics in `creamui-core` and `creamui-render`.
  `cargo clippy -p creamui-render --all-targets -- -D warnings` currently fails
  on complex callback types, large enum variants, macro paths and other style
  diagnostics. Keep public API and renderer allocation changes separate from
  mechanical lint cleanup.
