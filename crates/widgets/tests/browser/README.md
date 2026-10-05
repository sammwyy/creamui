# Browser bridge tests

The WASM fixture exercises themed file picker and controlled editor callbacks
in Chromium. Tests cover extension filters, selection, repeated selection,
cancellation and DOM cleanup, deferred UTF-8 file reads, clipboard copy/paste,
permission rejection, and paste replies after rebuilds or newer edits.

From the repository root, with `wasm-pack`, Node.js 22, and the Rust WASM target
installed:

```sh
wasm-pack build crates/widgets/tests/browser --target web --dev --out-dir pkg --out-name bridges
npm ci --prefix crates/widgets/tests/browser
cd crates/widgets/tests/browser
npx playwright install chromium
npm test
```

`CHROMIUM_BIN` selects an existing Chromium executable. `CREAMUI_BROWSER_PKG`
selects an absolute path to an already-built fixture package. The HTTP server
runs on localhost and closes with the browser after each run.
