# Components

CreamUI separates component behavior from visual opinion.

- **Themed widgets** such as `Button`, `TextInput`, and `ScrollView` read their tokens from a `Theme`.
- **Raw widgets** such as `RawButton`, `RawText`, and `RawScrollView` expose colors, radii, styles, and callbacks directly.

This makes it practical to start with the standard visual language and customize only the parts your application needs.

| Area | Themed components | Raw building blocks |
|---|---|---|
| Content | `Text`, `Heading`, `Card`, `Image` | `Block`, `Flex`, `Grid`, `RawText` |
| Actions | `Button` | `RawButton` |
| Text input | `TextInput`, `TextArea` | `RawTextInput`, `RawTextArea` |
| Values | `Checkbox`, `Switch`, `Slider` | `RawCheckbox`, `RawSwitch`, `RawSlider` |
| Selection | `Select`, `ListBox`, `RadioGroup`, `SegmentedControl` | Raw selection controls |
| Navigation | `Tabs`, `Sidebar`, `TreeView`, `Table` | Raw navigation and data-view controls |
| Feedback | `ProgressBar`, `ProgressRing`, `Popover`, `AlertDialog` | Raw feedback controls |
| Structured input | `DateInput`, `TimeInput`, `ColorPicker`, `FilePicker` | `RawDateTimePicker`, `RawColorPicker`, `RawFilePicker` |

## Style support

Every built-in Rust widget implements `Styled`. Layout declarations and box paint (`background`, `border`, `corner_radius`, and `outline`) are applied by the shared scene pipeline. A widget additionally reads only the style fields relevant to its own content.

| Component family | Layout and box paint | Typography | Interaction patches |
|---|---|---|---|
| `Block`, `Flex`, `Grid`, `RawView`, drag area | Yes | No | Box paint when the widget has the matching state |
| `RawText`, `Text`, `Heading`, `Link`, `Quote`, `Pre` | Yes | Color, family, size, alignment, and text decoration | Box paint when applicable |
| Buttons, toggles, sliders, inputs, pickers | Yes | Only text rendered by that widget | Hover, pressed, focus, or disabled paint when that state exists |
| Navigation, selection, lists, tables, feedback, scroll | Yes | Only labels or text rendered by that widget | State paint where the component exposes that state |
| `Image` | Yes | No | Box paint when applicable |

Widget-specific configuration remains explicit: a `Button`'s variant, an image's fit, or a text input's placeholder is not inferred from common styles. For an application widget, store the `Style` in the widget and implement `Styled::set_style` once; the shared builders then preserve its concrete type.

JSX only applies common style props to built-in intrinsic tags. Application components remain typed Rust functions: `<ProfileCard title={...} />` is checked against `ProfileCardProps`, and a `style` prop exists only if that function declares it. JSX does not inject styles into application-component props.

## Controlled state

Inputs are controlled by application state. Read a signal during build and write through the supplied callback.

```rust
let name = Signal::new(String::new());
let set_name = name.clone();

jsx! {
    <TextInput theme={&theme} value={name.get()}
        on_change={move |next| set_name.set(next)} />
}
```

Controllers are available for controls whose interaction state is larger than one value, including text inputs, tabs, scroll views, date/time pickers, and color pickers.

## Keyboard behavior

Buttons, checkboxes, switches, and sliders support focus navigation and keyboard activation. Text inputs support editing, selection, and caret state. Use Tab and Shift+Tab to move through focusable controls.

`Checkbox`, `Switch`, and their `Raw*` equivalents animate checked-state
changes by default with a 160 ms ease-out transition. The renderer retains
the visual state across reactive rebuilds, and rapid toggles reverse from
the current position. Initial mounting uses the supplied state immediately.

Use `.transition(...)` to adjust the duration and easing or disable animation:

```rust
use creamui_core::{Easing, Transition};
use creamui_widgets::{Checkbox, Switch};
use std::time::Duration;

let switch = Switch::new(false, || {})
    .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Linear));
let checkbox = Checkbox::new(true, || {}).transition(Transition::NONE);
```

Custom widgets can implement `Widget::has_transition` and
`Widget::paint_transition`, sampling the renderer-owned `TransitionState`
with `state.value(target, transition, painter)`. Frames are requested only
while the value is moving. Custom painters provide `Painter::frame_time`
to enable transitions; painters without a clock use the target immediately.

`Select::searchable()` keeps its search field and scrollable options inside
the popup at the control's resolved width. Popovers use opaque surfaces and
shield pointer events from the content behind them. Custom popup surfaces
can implement `Widget::blocks_pointer`; custom painters honor
`Painter::set_pointer_enabled` when determining hover and pressed state.
Absolutely positioned widgets float in the portal pass by default;
`Widget::is_portal` can return `false` for local decorations such as scrollbars.

## File pickers

Desktop and browser `FilePicker` use native selection dialogs. Android uses
a themed path prompt. Keep one `FilePickerController` per window and build the application's
content inside its host on every reactive rebuild:

```rust
use creamui_widgets::{FilePicker, FilePickerController};

let prompt = FilePickerController::new();
let selected = creamui_reactive::Signal::new(String::new());
let set_selected = selected.clone();
let root = prompt.host(|| {
    Box::new(
        FilePicker::new(selected.get(), move |file| {
            set_selected.set(file.path().map_or_else(
                || file.name().to_owned(),
                |path| path.display().to_string(),
            ));
        })
        .title("Open an image")
        .filter("Images", ["png", "jpg"]),
    )
});
```

Android pickers read the hosted controller from context. `.prompt(&prompt)`
selects the same modal explicitly on desktop, which is useful for a custom
workflow or testing. Applications with their own root overlay can instead
provide the controller in context and place `FilePrompt::new(&prompt)` after
application content in paint order.

The prompt retains edits across rebuilds, focuses its input when opened,
confines Tab navigation and pointer input to the modal, and reports validation
errors without calling `on_change`. Enter or Open accepts a readable regular
file with an allowed extension; `*` allows any extension. Cancel, Escape, or a
backdrop click closes it without changing the selection. Android Back dismisses
the keyboard first. Paths must already be accessible to the application; the
prompt does not grant storage permissions or resolve document-provider URIs.

Selection callbacks receive `SelectedFile`. `name()` returns the filename;
`path()` is `Some` for desktop/Android selections and `None` for browser files.
`read_bytes().await` reads data asynchronously on every platform, preserving
I/O errors. Browser selections keep the file handle without loading its
contents until requested. Canceling the dialog preserves the current value.

Browser text editors use the asynchronous clipboard API. Use controlled
editors with `TextController` when edits must survive reactive rebuilds.
Pending pastes are discarded after a new edit, selection/cursor change, or
newer paste request. Paste respects the controller's change guard. Clipboard
access requires a secure context and the browser's permission or activation
policy.

## Retained runtime composition

`creamui_core::runtime::MountCx` constructs nodes and scopes their bindings
under an `Owner`. `branch_when` tracks a derived condition; `switch` mounts one
of two variants; `keyed_with` tracks a derived `Vec<T>` and retains rows by key.
These APIs mount roots beside static siblings without adding layout wrappers.
Their return value is a hidden anchor identifying the region.

Keyed render callbacks receive `Signal<T>` rather than `&T`. Read that signal
inside a binding to update a surviving row, or inside its event handler to read
the current value. Values require `Clone + PartialEq`; unchanged values do not
rerun row bindings. Duplicate keys panic before changing the mounted tree.

```rust
use creamui_core::runtime::{MountCx, Mutation};
use creamui_reactive::Signal;

fn mount_rows(cx: &MountCx, rows: Signal<Vec<(u64, String)>>) {
    cx.text("Before");
    cx.keyed(rows, |row| row.0, |cx, row| {
        let node = cx.text("");
        cx.bind(move |tx| {
            tx.apply(Mutation::SetText {
                node,
                text: row.get().1.into(),
            });
        });
        node
    });
    cx.text("After");
}
```

Branch and row callbacks receive a detached root context. Use
`cx.with_parent(root)` to mount descendants of that root. Source reads are
tracked separately from content bindings, so changing a row's content does not
remount its surrounding branch. Effects retain the context scopes active when
they were created, including after a branch is mounted by a later signal write.
`creamui_reactive::untrack` reads values without subscribing the surrounding
effect while allowing nested effects to track their own reads.

Call `Owner::dispose` when removing an owned view. Disposal stops all descendant
effects before running child-first cleanup and removes branch/list nodes and
anchors. Dropping an owner stops its effects but does not run explicit cleanup.
`WeakOwner` upgrades only while its scope is active. Effects queued by nested
writes run after the current effect finishes; writes outside an effect or batch
finish their notifications synchronously.

### Owned window views

`View::new` stores a mount callback until the window provides its `MountCx`
and hook context. Return a `View` from the normal `run`, `AppBuilder::window`,
`AppHandle::append_window`, or popup builder to mount once. Widget results keep
their reactive rebuild behavior.

```rust
use creamui_core::runtime::{Mutation, View};
use creamui_render::{AppBuilder, WindowOptions, use_viewport};
use creamui_theme::{Color, use_theme};

let app = AppBuilder::new().window(
    WindowOptions::default(),
    Color::rgb(0, 0, 0),
    |_| {},
    |_| View::new(|cx| {
        let root = cx.container();
        cx.bind(move |tx| {
            let viewport = use_viewport();
            let theme = use_theme();
            tx.apply(Mutation::SetLayoutStyle {
                node: root,
                style: creamui_core::Style::new()
                    .width(viewport.width).height(viewport.height).layout,
            });
            tx.apply(Mutation::SetPaintStyle {
                node: root,
                style: creamui_core::PaintStyle {
                    background: Some(theme.colors.surface.into()),
                    ..Default::default()
                },
            });
        });
        root
    }),
);
```

The window owns the binding scope, preserves it while native surfaces are
suspended, and disposes it on close. Keeping a window handle or runtime clone
does not keep the view's bindings active after closing. Dynamic root branches
and lists share a stable host root. Mount failures release partial nodes and
bindings; an application's registered panic handler permits an empty fallback.

For a view without a window, `MountedView::new(view)` creates its runtime and
scope. `runtime()`, `owner()`, and `root()` expose the retained state. Dispose
or drop the `MountedView` to release its nodes and bindings. An externally
supplied `AppBuilder::runtime_window` continues to use application-owned state.

Set `CREAMUI_DUMP_JSX=1` while compiling to inspect expanded Rust for `jsx!`,
`abi_jsx!`, and `#[component]`. Empty and `0` values disable the dump.

## Composable dialogs

`Dialog::custom` accepts arbitrary header, content and footer widgets. The themed
`DialogHeader`, `DialogContent`, `DialogFooter`, `DialogMessage` and `ModalActions`
provide defaults without restricting the content to a fixed confirmation layout.
`ModalActions` gives each child the same share of the footer width, with a small
configurable gap. Its children may be any styled widgets, including custom buttons.

```rust
use creamui_widgets::{
    Button, ButtonSize, ButtonState, ButtonVariant, Dialog, DialogContent,
    DialogFooter, DialogHeader, DialogMessage, DialogPlacement, ModalActions,
};

let dialog = Dialog::custom(on_cancel)
    .width(420.0)
    .placement(DialogPlacement::TopLeft)
    .header(Box::new(DialogHeader::new("Choose a location")))
    .content(Box::new(DialogContent::new()
        .child(Box::new(DialogMessage::new("Enter the destination below.")))
        .child(Box::new(input))))
    .footer(Box::new(DialogFooter::new().child(Box::new(
        ModalActions::new()
            .child(Button::styled(ButtonVariant::Secondary, ButtonSize::Md,
                "Cancel", ButtonState::Normal, on_cancel_button))
            .child(Button::styled(ButtonVariant::Primary, ButtonSize::Md,
                "Continue", ButtonState::Normal, on_continue))
    ))));
```

`DialogHeader` supports an icon, a description and a trailing close control.
`DialogMessage::icon` adds an icon alongside explanatory text. Section styles can
be overridden with the common `Styled` builders. `DialogPlacement` supports center,
top, left and top-left placement. Backdrop dismissal is built in; applications
should route Escape through the same dismissal callback in their window handler.
The existing `Dialog::new(title, message, dismiss)` and `AlertDialog` APIs use the
same components for simple messages and confirmations.

## Command palettes

`CommandPalette` supplies a shared compact search header, grouped result rows,
shortcut badges, automatic result height and a scrollable list. It uses `Dialog`
for its surface. Applications provide command metadata and callbacks:

```rust
use creamui_widgets::{CommandPalette, CommandPaletteController, CommandPaletteItem};

let controller = CommandPaletteController::new(); // Keep this in window state.
let commands = vec![
    CommandPaletteItem::new("open", "Open file", on_open)
        .category("File")
        .shortcut("Ctrl+O"),
];
let palette = CommandPalette::new(&controller, commands, on_dismiss).viewport(size);
```

The controller owns query, selection and scroll state. Matching includes command
IDs, labels, categories and shortcut text. Arrow keys select results; Enter invokes
the selected callback; Escape dismisses. Query changes reset selection and scroll.
Use `.placeholder`, `.hint`, `.width` and `.max_results_height` to configure the
surface. `.reference(true)` presents a shortcut reference with the same layout.

Applications that intercept native keys before widget dispatch can forward core
`KeyInput` values to `CommandPaletteController::handle_key`; this path does not
require a theme context. Call `reset()` when opening the palette and focus the
window's text input after the palette becomes visible.
