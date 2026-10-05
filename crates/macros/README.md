# creamui-macros

JSX syntax and typed component helpers for CreamUI.

```rust
use creamui_macros::jsx;
use creamui_core::layout::FlexDirection;
use creamui_widgets::layout::{Align, Justify};

let screen = jsx! {
    <Flex direction={FlexDirection::Column} gap={12.0}
        align={Align::Center} justify={Justify::Center}>
        <Button on_click={|| save()}>"Save"</Button>
    </Flex>
};
```

The macro expands to ordinary Rust constructors, so component names, imports, props, and callback types are checked by the compiler.

Set `CREAMUI_DUMP_JSX=1` when compiling to print generated Rust for `jsx!`,
`abi_jsx!`, and `#[component]` to stderr. An unset, empty, or `0` value disables
output. Cargo tracks changes to this setting and recompiles the macros.

```sh
CREAMUI_DUMP_JSX=1 cargo check -p hello-world
```
