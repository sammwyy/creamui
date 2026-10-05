# Showcase web demo

Build every demo under `demo/` (this one included) and serve them statically:

```sh
../build.sh   # or: demo/build.sh from the repo root
../serve.sh   # or: demo/serve.sh from the repo root
```

Then open <http://localhost:8080/showcase/>. The page gets a canvas from
`winit`, sized to fill the whole browser window via CSS; it
uses CreamUI's normal widget tree and interactions, so theme, navigation,
inputs, sliders, tabs, scrolling and file selection work in the browser.

The web bundle includes Liberation Sans under the SIL Open Font License
(`assets/OFL-LiberationSans.txt`). Browser WASM cannot load system font files,
so the demo registers this face before building its first widget tree.

File pickers return browser-backed `SelectedFile` handles whose bytes can be
read asynchronously. Text inputs and text areas use asynchronous browser
clipboard APIs. Serve through localhost or HTTPS; clipboard access follows
the browser's permission and user-activation policy.
