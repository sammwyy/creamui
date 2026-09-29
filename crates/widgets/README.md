# creamui-widgets

The raw and themed component library for CreamUI.

Use themed widgets such as `Button`, `TextInput`, `ScrollView`, and `ColorPicker` for the standard CreamUI appearance. Use the `Raw*` equivalents when your application needs to provide colors, radii, and interaction details directly.

`VirtualList` builds only visible rows. See `examples/virtual-list` for a large, mixed-height list.

```rust
use creamui_widgets::Button;

let save = Button::new(&theme, "Save", || save_document());
```

See the [component guide](https://github.com/sammwyy/creamui/blob/main/docs/components.md) for the full component map.
