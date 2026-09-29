# Showcase web demo

Build every demo under `demo/` (this one included) and serve them statically:

```sh
../build.sh   # or: demo/build.sh from the repo root
../serve.sh   # or: demo/serve.sh from the repo root
```

Then open <http://localhost:8080/showcase/>. The page gets a canvas from
`winit`, sized to fill the whole browser window via CSS; it
uses CreamUI's normal widget tree and interactions, so theme, navigation,
inputs, sliders, tabs, scrolling and pickers that do not require host APIs
work in the browser.

The web bundle includes Liberation Sans under the SIL Open Font License
(`assets/OFL-LiberationSans.txt`). Browser WASM cannot load system font files,
so the demo registers this face before building its first widget tree.

The native file dialog and synchronous clipboard integration are intentionally
unavailable on WASM. A browser file/clipboard bridge needs asynchronous web
APIs and is outside this small standalone demo.
