# CreamUI

**A native, reactive UI framework for Rust desktop applications.**

CreamUI pairs a familiar component model with native windows, CSS-like flex and grid layout, JSX syntax, and a configurable application theme. It is built for apps that want a small Rust-native UI stack without embedding a web view.

> **Project status:** early MVP. The public API is evolving; Linux is the currently supported desktop target.

## Why CreamUI?

- Native desktop windows through `winit`, with GPU or CPU presentation.
- Reactive state with small, straightforward `Signal` values.
- Headless raw widgets and polished themed widgets in the same toolkit.
- JSX syntax that compiles to ordinary Rust widget builders.
- Flex and grid layouts powered by Taffy.
- Accessible keyboard interactions for the standard controls.
- Built-in date, time, color, and native file pickers.
- PNG by default, with opt-in JPEG and WebP image decoding.
- An optional C ABI for shared runtimes and non-Rust hosts.

## Components

CreamUI includes text, buttons, text inputs, text areas, checkboxes, switches, sliders, select/list/radio controls, tabs, sidebars, scroll views, trees, tables, popovers, dialogs, progress indicators, pickers, and images.

Every visual component has a lower-level `Raw*` counterpart where applications can choose colors, radii, spacing, and event behavior themselves.

## Quick start

Add the runtime crates to your application:

```toml
[dependencies]
creamui = { version = "0.1", features = ["jsx"] }
```

Then create a window and return a widget tree:

```rust
use creamui::widgets::layout::{Align, Flex, Justify};
use creamui::{run, BoxedWidget, Button, Signal, Size, Text, Theme, WindowOptions};

fn main() {
    let clicks = Signal::new(0_u32);

    run(
        WindowOptions {
            title: "Hello CreamUI".into(),
            width: 480,
            height: 320,
            ..Default::default()
        },
        Theme::default().surface,
        |_| {},
        move |viewport: Size| -> BoxedWidget {
            let theme = Theme::default();
            let increment = clicks.clone();
            Box::new(
                Flex::column()
                    .size(viewport.width, viewport.height)
                    .gap(12.0)
                    .justify(Justify::Center)
                    .align(Align::Center)
                    .background(theme.surface)
                    .child(Box::new(Text::new("Hello, CreamUI!").font_size(28.0)))
                    .child(Box::new(Text::new(format!("Clicked {} times", clicks.get()))))
                    .child(Box::new(Button::new("Click me", move || increment.update(|n| *n += 1)))),
            )
        },
    );
}
```

## Explore the examples

```sh
cargo run -p hello-world
cargo run -p showcase
cargo run -p pickers
cargo run -p images
cargo run -p flex
cargo run -p jsx-styles
cargo run -p tray
```

The showcase is the fastest way to explore the available controls and theme behavior.
The `tray` example shows a persistent background application: closing its
window leaves the process alive, while the system tray can create it again,
update a reactive counter, or quit the process.

## Web showcase demo

The same showcase can be compiled to WASM and opened in a browser:

```sh
demo/build.sh          # builds every demo under demo/ to demo/<name>/pkg/
demo/serve.sh          # serves demo/ statically at http://localhost:8080
```

Open <http://localhost:8080/showcase/>. Its canvas fills the whole page and
takes mouse/keyboard input like a native window. See
[`demo/showcase`](demo/showcase) for the browser-specific limitations of
native file dialogs and clipboard access, and `demo/build.sh --dev` for a
faster, unoptimized build while iterating.

## Documentation

- [Getting started](docs/getting-started.md)
- [Styling scope](docs/styling.md)
- [Components](docs/components.md)
- [Theming](docs/theming.md)
- [Images](docs/images.md)
- [Dynamic and C ABI usage](docs/ffi.md)
- [Release guide](docs/releasing.md)

## Workspace crates

| Crate | Use it for |
|---|---|
| `creamui` | Main facade crate; the recommended starting point |
| `creamui-reactive` | Signals and reactive effects |
| `creamui-theme` | Default theme, light palette, and design tokens |
| `creamui-core` | Widget, painter, scene, and layout abstractions |
| `creamui-widgets` | Raw and themed components |
| `creamui-image` | PNG, JPEG, and WebP image widgets |
| `creamui-render` | Native windows and frame presentation |
| `creamui-tray` | Optional native system-tray backends |
| `creamui-devtools` | Development-only FPS/frame-time/CPU/RAM overlay (F3) |
| `creamui-macros` / `creamui-jsx` | JSX syntax and component support |
| `creamui-abi`, `creamui-ffi`, `creamui-dynamic` | Optional dynamic-runtime and C ABI integration |

## License

CreamUI is licensed under [Apache-2.0](LICENSE-APACHE). It permits commercial, private, and open-source use.

CreamUI bundles DejaVu Sans regular and bold under the permissive Bitstream Vera license; the notice ships with the crates that use those fonts.

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request.
