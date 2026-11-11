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
- Reclaim a Wayland SHM buffer released after a resize even when the window
  does not present another frame. Pools are currently reclaimed on the next
  presentation.
- Improve scrolling when a framebuffer shift is unavailable. The CPU blits
  only when one scroll layer moves by whole physical pixels over a solid,
  opaque backdrop; otherwise it repaints the layer viewport.

## Pending TODO components

Proposed additions, **not implemented APIs**. Compared with the
[GPUI Kit component catalog](https://gpui-kit.com/component/) and
[its component index](https://gpui-kit.com/llms.txt). The inventory of what
CreamUI already provides, including raw counterparts and composite widgets,
is in [docs/components.md](docs/components.md#component-catalog).
Prefer reusable behavior in a headless `Raw*` widget (or a small set of raw
parts) with explicit state/callbacks, then a `Theme`-driven wrapper. Names
below are working names; do not introduce a second widget where an existing
one can be extended. Variants are design suggestions, not promised features.

### Priority: common application UI

| Name | Purpose | Raw plan | Themed plan and variants |
|---|---|---|---|

- Add motion and compact spacing options to `Collapsible`, and pulse/reduced-motion
  modes to `Skeleton`. Both currently render without animation.
- Add roving heading focus and bordered/flush layouts to `Accordion`, horizontal
  labels to `Field`, multi-column layout to `Form`, and page-size selection to
  `Pagination`.
- Add a shared focus ring and toolbar layout to `InputGroup`, grouped styles
  to `ToggleButton`, and a built-in menu host to `DropdownButton`. The split
  button currently delegates menu presentation to its callback.
- Add range selection, multi-month layout, and localized month/weekday labels
  to `Calendar`.
- Add grouped digit spacing and platform-native mobile numeric keyboard hints
  to `OtpInput`.
- Add a visible timeout progress indicator and pause-on-hover behavior to
  `Toast`.

### Priority: workspace and data

| Name | Purpose | Raw plan | Themed plan and variants |
|---|---|---|---|
| `Dock` | Arrange persistent tab groups and split/edge panels in a workspace. | `RawDock` models drag, drop, nested layouts, and serializable state on raw tabs/splits. | `Dock` themes drop targets, tabs, and panels; optional edge docks. |
| `HoverCard` | Show richer interactive preview content than a tooltip. | `RawHoverCard` shares anchored positioning with `RawTooltip` but allows focusable content. | `HoverCard` themes preview surface; delayed hover/focus. |

- Add dividers to `DescriptionList`, compact height to `StatusBar`, and
  connectors and custom step icons to `Stepper`.

### Priority: specialized content

| Name | Purpose | Raw plan | Themed plan and variants |
|---|---|---|---|
| `CodeEditor` | Edit source with line numbers, highlighting, and large-file navigation. | `RawCodeEditor` builds on text editing with revisioned document, gutter, IME, and undo. | `CodeEditor` themes tokens/selection; read-only/editable and wrap/no-wrap. |
| `RichTextView` | Render application-provided Markdown or rich document nodes. | `RawRichTextView` lays out text runs, links, images, and selections safely. | `RichTextView` themes headings/links/code; Markdown as an optional parser. |

- Add stacked shortcut chords to `Kbd` and outlined and size variants to `Tag`.

These are intentionally distinct from existing `ColorPicker`, `TimeInput`,
`Select`/`ComboBox`, `CommandPalette`, `ContextMenu`, `VirtualList`,
`ProgressBar`/`ProgressRing`, `AppTitlebar`, `RawSplitView`, and
`RawMarquee`. Reuse those implementations before creating new names.
Design references: [Calendar](https://gpui-kit.com/component/calendar/),
[Dock](https://gpui-kit.com/component/dock/),
[Diff](https://gpui-kit.com/component/diff/),
[StatusBar](https://gpui-kit.com/component/status-bar/).

## Tooling

- Add raw, manually styled, and themed visual coverage for every exported widget
  not yet represented by the showcase, including overlays, virtualized data
  views, menus, titlebars, and pickers. Exercise narrow and high-DPI viewports,
  light/dark themes, keyboard focus, hover, scrolling, and animation states.
- Clear existing strict Clippy diagnostics in `creamui-core` and `creamui-render`.
  `cargo clippy -p creamui-render --all-targets -- -D warnings` currently fails
  on complex callback types, large enum variants, macro paths and other style
  diagnostics. Keep public API and renderer allocation changes separate from
  mechanical lint cleanup.
