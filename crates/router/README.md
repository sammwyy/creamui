# CreamUI Router

Enable `creamui`'s `router` feature, or depend directly on `creamui-router`.
Create a router **once**, outside the window builder:

```rust,ignore
let router = Router::builder("/")
    .route("/", home)
    .route("/users/:id", profile)
    .route("/files/*rest", files)
    .not_found(not_found)
    .build()?;
let provider = RouterProvider::new(router);

// Inside the window builder:
provider.render(|| {
    // Build sidebar, global layout and other contexts here.
    // Place RouterOutlet::render() wherever the page belongs.
    build_layout()
})
```

Components are `Fn() -> BoxedWidget` factories. The provider adds no visual
container. Build children inside its closure, so hooks resolve the correct
context. The outlet inherits contexts between it and the provider. Nested
providers own independent routers; the outer provider handles native Back.
Register complete paths for pages sharing a layout; nested route graphs are
not required to put the outlet deeply inside your layout.

`use_router()` returns a cloneable handle. `use_location()`, `use_params()` and
`use_query_params()` subscribe the current effect/window builder to navigation.
Capture the router in event callbacks; hooks are only available in a provider
scope. `navigate` pushes history, `replace` replaces the current entry, and
`back`, `forward`, `go` traverse it. Methods return `Result` and leave the
previous location intact if parsing or browser navigation fails.

Paths are case sensitive. Static segments outrank `:params`, which outrank
terminal `*wildcards`; ties use registration order. A plain `*` uses the key
`"*"`. Trailing slashes are ignored when matching. Params are percent decoded
after splitting the path, so `%2F` remains inside one parameter. Query pairs
are decoded separately, preserving duplicate keys. Relative navigation follows
URL resolution (from `/users/42`, `../settings` goes to `/settings`). Missing
routes render `not_found`, or an empty node when no fallback is registered.

## Platforms

On web, `Router::builder` starts from `window.location`, including query and
fragment. Navigation uses History API push/replace without a reload;
`popstate` updates the reactive router on browser Back/Forward. Traversal is
asynchronous. `can_go_back`/`can_go_forward` describe entries visited by this
router session; browsers don't expose the complete preexisting history.
Keep one browser-backed router per page; use memory routers for nested,
independent panels. Serve your app's HTML for internal paths such as
`/users/42` so refreshes and direct links work. Reading the initial URL does
not require server-rendered HTML.

On desktop and Android, the default is an in-memory URL/history. Explicit
`Router::memory` also works on web. In debug builds,
`CUI_ROUTER_DEFAULT_PATH=/users/42` overrides the configured initial path;
an empty value resolves to `/`. `.dev_override(false)` disables this.
For WASM memory routers the environment variable is read at **compile time**.
A browser-backed router always starts from the actual browser URL.

Android stays inside one Activity. With `creamui`'s `router` and
`platform-android` features, `AppBuilder::run_android` installs the adapter:

Use `singleTop` for the host Activity (the example sets this through
`package.metadata.android.application.activity.launch_mode`). Relaunching
the existing top Activity must not create a second native runtime in the
same process.

- System Back dismisses the IME first, then pops the router, then moves the
  Activity task to the background at its root. This preserves the native
  event loop and permits a warm resume when reopening the app, matching
  modern Android root navigation. It uses NativeActivity's key event path.
- The outer provider restores and saves history using app-private
  SharedPreferences (`creamui-router`, key `main`). This also restores history
  across a process restart. Configure a separate key with
  `creamui_router::android::attach(&router, app, "my-router")` before building
  the provider. Debug env overrides skip automatic restoration.
- The initial Intent data URI becomes an internal path/query/fragment and
  takes precedence over restored history. Set manifest intent filters for
  your URI scheme or HTTPS host; verified App Links also require the host's
  `assetlinks.json` association.
- To receive subsequent links in a reused Activity, its Java/Kotlin host
  must call `setIntent(intent)` from `onNewIntent`. The runtime then checks
  for new Intent data. Alternatively forward the URI explicitly through
  `Router::apply_deep_link`. Stock NativeActivity does not expose
  `onNewIntent` to Rust. A minimal single-Activity host is provided in
  `android/CreamRouterActivity.java`; point the manifest's activity name at
  it and compile it with your Android build system.

`save_history`/`restore_history` are available for manual lifecycle integration
on memory routers. Browser history belongs to the browser and cannot be
restored with those methods. This crate does not create Activities or install
Jetpack Navigation. Predictive Back animations require host-level Android
integration beyond NativeActivity's key events.

See `examples/router` for a sidebar, global layout context, params, queries,
wildcards and history controls.
