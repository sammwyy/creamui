# Router example

```sh
cargo run -p router
CUI_ROUTER_DEFAULT_PATH='/users/42?tab=settings' cargo run -p router
```

The sidebar pushes actual routes. The central outlet renders pages below a
layout-level context, while URL, params and query values update reactively.
Back, Forward and Replace demonstrate the memory/browser history APIs.

Build for web:

```sh
wasm-pack build examples/router --target web --dev --out-dir pkg
```

Serve `examples/router/index.html` and its generated `pkg` directory; the
router automatically uses the page URL. Configure the server to fall back
to `index.html` for `/users/42`, `/files/...` and other internal paths. The
WASM entry point reuses the repository's bundled showcase font.

For Android, build the library with `--no-default-features --features
platform-android` using cargo-apk and the Android toolchain. The same example
uses one NativeActivity and in-memory routing with system Back and persisted
history. See `crates/router/README.md` for deep-link host integration.

To reproduce the device checks (Python 3 and ADB):

```sh
cargo apk build --manifest-path examples/router/Cargo.toml --lib \
  --target aarch64-linux-android --no-default-features --features platform-android
adb -s DEVICE install --no-incremental -r target/debug/apk/creamui_router_example.apk
python3 examples/router/check-android.py --device DEVICE
```

Keep the device unlocked and in portrait orientation, without touching it
during the checks. The script does not clear application data. It checks
sidebar navigation, params/query routes, Back/Forward, replace, wildcards,
not-found pages, process recreation, cold Intent deep links, backgrounding
at the history root, and warm resume with navigation afterward. It stops before sending input if another app
or a system overlay owns the foreground. Cold deep links are sent to the
Activity explicitly; external-link resolution still requires manifest filters.
