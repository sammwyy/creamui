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
