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
| `Accordion` | Group independently expandable sections. | `RawAccordion` manages item expansion, headings, and keyboard navigation without colors. | `Accordion` themes headers and panels; single/multiple-open, bordered/flush. |
| `Toast` | Queue transient nonmodal notifications with timeout and dismissal. | `RawToast` plus a host/controller handle stacking, timers, focus, and close actions. | `Toast` uses success/error/info variants, action and progress options. |
| `Tooltip` | Show brief help on hover or keyboard focus. | `RawTooltip` handles trigger, delay, pointer/focus lifecycle, and viewport-aware placement. | `Tooltip` uses theme surface and optional shortcut text; top/bottom/side positions. |
| `Field` | Associate a field name, hint, required state, and errors with an input. | `RawField` composes labels and content with explicit semantics and validation state. | `Field` themes labels/help/errors; horizontal/stacked layouts. |
| `Form` | Group fields, validation results, and submission state. | `RawForm` composes `RawField` and controlled input callbacks; no implicit storage. | `Form` themes spacing and summary errors; single/two-column layouts. |
| `InputGroup` | Attach prefixes, suffixes, icons, or buttons to existing text inputs. | `RawInputGroup` arranges focus-aware addons around `RawTextInput`/`RawTextArea`. | `InputGroup` shares borders and focus ring; inline/toolbar variants. |
| `NumberInput` | Edit bounded numbers using typing, step buttons, and keyboard increments. | `RawNumberInput` parses/formats controlled values, clamps on commit, and exposes step actions. | `NumberInput` themes steppers; integer/decimal, compact/regular. |
| `OtpInput` | Enter a fixed-length one-time code with predictable paste and focus. | `RawOtpInput` owns cell navigation, paste distribution, and controlled value. | `OtpInput` themes focus/error states; grouped or uniform cells. |
| `Calendar` | Browse months and select a date independently of the existing date input. | `RawCalendar` handles date grid, disabled days, locale/week start, and keyboard navigation. | `Calendar` styles today/selected/range days; single/range and multi-month. |
| `Pagination` | Navigate paged datasets with current page and total count. | `RawPagination` calculates page links, ellipses, and previous/next actions. | `Pagination` themes buttons; compact/full and page-size option. |
| `ToggleButton` | Expose on/off pressed state as a button, not a checkbox or switch. | Extend `RawButton` with explicit pressed state and accessible activation. | `ToggleButton` themes icon/text and single/grouped styles. |
| `DropdownButton` | Keep a primary action separate from an adjacent menu trigger. | Compose `RawButton` with raw menu behavior and split focus handling. | `DropdownButton` themes joined/split borders and small/regular sizes. |

- Add motion and compact spacing options to `Collapsible`, and pulse/reduced-motion
  modes to `Skeleton`. Both currently render without animation.

### Priority: workspace and data

| Name | Purpose | Raw plan | Themed plan and variants |
|---|---|---|---|
| `DataTable` | Add sorting, filtering, selection, and column management to large tables. | Extend `RawTable`/`RawVirtualTable` with controlled column, row, and sort models. | `DataTable` themes headers and selection; dense/comfortable and virtualized. |
| `DescriptionList` | Present aligned key/value details, not editable rows. | `RawDescriptionList` handles term/value layout and wrapping. | `DescriptionList` themes labels/dividers; horizontal/stacked. |
| `ResizablePanels` | Resize multiple panels with handles and min/max constraints. | Extend `RawSplitView`/`SplitState` for nested panels, keyboard resize, and persistence. | `ResizablePanels` themes handles; horizontal/vertical and collapsible. |
| `Dock` | Arrange persistent tab groups and split/edge panels in a workspace. | `RawDock` models drag, drop, nested layouts, and serializable state on raw tabs/splits. | `Dock` themes drop targets, tabs, and panels; optional edge docks. |
| `StatusBar` | Display left/center/right contextual status at the window edge. | `RawStatusBar` lays out slotted content without deciding text or actions. | `StatusBar` themes separators and compact/regular heights. |
| `Toolbar` | Arrange commands in a header or editor with overflow handling. | `RawToolbar` owns group ordering, focus navigation, and overflow. | `Toolbar` themes icon/text actions; attached/floating variants. |
| `Stepper` | Show and navigate stages in a workflow. | `RawStepper` exposes active/completed/disabled step state and activation. | `Stepper` themes connectors; horizontal/vertical, numbered/icon. |
| `Sheet` | Reveal contextual content from an edge, separate from centered dialogs. | `RawSheet` handles overlay placement, focus return, dismissal, and scrolling. | `Sheet` themes panel/backdrop; left/right/bottom and modal/nonmodal. |
| `HoverCard` | Show richer interactive preview content than a tooltip. | `RawHoverCard` shares anchored positioning with `RawTooltip` but allows focusable content. | `HoverCard` themes preview surface; delayed hover/focus. |
| `Carousel` | Browse a finite sequence of slides by keyboard, pointer, or controls. | `RawCarousel` manages index, clipping, snap behavior, and reduced motion. | `Carousel` themes arrows/indicators; contained/full-bleed. |
| `GridLayout` | Rearrange and resize dashboard cards with drag handles. | `RawGridLayout` exposes controlled item coordinates, collision rules, and bounds. | `GridLayout` themes drag previews and handles; editable/locked. |

### Priority: specialized content

| Name | Purpose | Raw plan | Themed plan and variants |
|---|---|---|---|
| `Kbd` | Display platform-aware keyboard shortcuts in help or menus. | `RawKbd` formats key sequences without prescribing keycap appearance. | `Kbd` themes keycaps; inline/stacked chords. |
| `Tag` | Show a category label, optionally removable or selectable. | `RawTag` exposes label, optional close action, and selection state. | `Tag` themes neutral/accent tones; outlined/filled and sizes. |
| `Rating` | Choose a rating by keyboard or pointer, including read-only display. | `RawRating` handles bounded values and focus/pointer hit regions. | `Rating` themes stars; interactive/read-only and half-step. |
| `Chart` | Plot application-provided series without embedding a web view. | `RawPlot` draws axes/series with explicit scales, clipping, and hit tests. | `Chart` themes grid/legend; line/bar/area first, other types later. |
| `Diff` | Review supplied patch hunks without editing source. | `RawDiff` virtualizes lines/hunks with gutter, selection, and conflict markers. | `Diff` themes insert/delete/neutral lines; unified/side-by-side. |
| `CodeEditor` | Edit source with line numbers, highlighting, and large-file navigation. | `RawCodeEditor` builds on text editing with revisioned document, gutter, IME, and undo. | `CodeEditor` themes tokens/selection; read-only/editable and wrap/no-wrap. |
| `RichTextView` | Render application-provided Markdown or rich document nodes. | `RawRichTextView` lays out text runs, links, images, and selections safely. | `RichTextView` themes headings/links/code; Markdown as an optional parser. |
| `Attachment` | Show file or media previews and upload/download lifecycle. | `RawAttachment` composes preview, progress, error, and action slots. | `Attachment` themes compact/card and pending/failed/complete states. |
| `ChatMessage` | Group sender, body, timestamp, and actions in a conversation. | `RawChatMessage` arranges arbitrary message content and metadata. | `ChatMessage` themes incoming/outgoing and compact/expanded layouts. |
| `MessageScroller` | Follow the tail of a long conversation and preserve position on prepend. | Extend `RawVirtualList` with anchored scrolling, unread marker, and jump-to-latest. | `MessageScroller` themes jump/unread affordances; follow/pause states. |
| `Questionnaire` | Navigate multi-step questions with validation and branching. | `RawQuestionnaire` models steps, answers, and controlled navigation. | `Questionnaire` composes `Form`/`Stepper`; linear/branching flows. |

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
