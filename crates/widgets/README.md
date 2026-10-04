# creamui-widgets

The raw and themed component library for CreamUI.

Use themed widgets such as `Button`, `TextInput`, `ScrollView`, and `ColorPicker` for the standard CreamUI appearance. Use the `Raw*` equivalents when your application needs to provide colors, radii, and interaction details directly.

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

`FilePicker` uses native desktop dialogs and a hosted Android path prompt.
Keep a `FilePickerController` per window and wrap application content with
`controller.host(...)` in the reactive build closure. See the
[file picker guide](../../docs/components.md#file-pickers) for context,
validation, and browser support.
