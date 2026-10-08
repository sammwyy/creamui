# creamui-widgets

The raw and themed component library for CreamUI.

Use themed widgets such as `Button`, `TextInput`, `ScrollView`, and `ColorPicker` for the standard CreamUI appearance. Use the `Raw*` equivalents when your application needs to provide colors, radii, and interaction details directly.

`Checkbox` and `Switch`, including their raw versions, animate state changes
automatically. Configure them with `.transition(creamui_core::Transition::new(duration))`
or disable animation with `.transition(creamui_core::Transition::NONE)`.

`VirtualList` builds only visible rows. See `examples/virtual-list` for a large, mixed-height list.
`VirtualTable` uses the same `VirtualListState` and loads cell text only for
visible rows while keeping the header fixed above the scrolling body.
`VirtualTreeView` flattens expanded nodes and mounts only rows near the
viewport, retaining node IDs as row keys.

```rust
use creamui_widgets::Button;

let save = Button::new("Save", || save_document());
```

See the [component guide](https://github.com/sammwyy/creamui/blob/main/docs/components.md) for the full component map.

`FilePicker` uses native desktop and browser dialogs and a hosted Android
path prompt. Selection callbacks receive `SelectedFile`: `name()` returns
its filename, `path()` returns a native path when available, and
`read_bytes().await` reads data without blocking the UI thread.
Keep a `FilePickerController` per window and wrap application content with
`controller.host(...)` in the reactive build closure. See the
[file picker guide](../../docs/components.md#file-pickers) for context,
validation, and browser support.

Browser text inputs and text areas support asynchronous clipboard shortcuts.
`TextInput::controlled` and `TextArea::controlled` retain paste targets across
rebuilds through their `TextController`, discarding a pending result when
editing state changes. Clipboard access requires HTTPS or localhost and the
browser's permission or activation policy.
