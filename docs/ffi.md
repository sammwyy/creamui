# Dynamic and C ABI usage

Most Rust applications should use the native crates directly. CreamUI also offers an optional ABI path for applications that need to load a shared runtime at process startup or from another language.

- `creamui-abi` contains the plain `#[repr(C)]` value types.
- `creamui-ffi` builds the `creamui` shared library and exports C functions.
- `creamui-dynamic` is the safe Rust client that loads that library with `libloading`.

The ABI is intentionally separate from the normal Rust widget API. It is useful for plugin hosts and shared desktop runtimes, but it is not required for ordinary native applications.

See the public Rust API documentation for `creamui-dynamic::AppBuilder` and the exported declarations in `creamui-ffi` when integrating this path.

For initial compositor blur, pass `CWindowOptionsV2` to `creamui_run_v2` or
`creamui_app_builder_add_window_v2`. Its `base` field is the original
`CWindowOptions`, and its `blur` field selects `CUI_BLUR_NONE`,
`CUI_BLUR_WINDOW`, or `CUI_BLUR_RECT`. Rectangle coordinates are logical pixels
relative to the window. Set `transparent` in the base options for blur to be
visible. The original entrypoints and struct layout remain available. The
dynamic Rust client accepts `WindowOptions { blur: Some(BlurRegion::Window),
..Default::default() }` or a rectangular `BlurRegion`.

For `CUI_PAINT_RADIAL_GRADIENT` and `CUI_PAINT_REPEATING_RADIAL_GRADIENT` paint
operations, `x` and `y` give the center, `radius` gives the horizontal radius,
and `angle_degrees` gives the vertical radius. For a circular gradient, the two
radii are equal.
Use `cui_radial_stop_count` and `cui_get_radial_stop` to read every resolved
color stop in a retained radial paint operation. The dynamic Rust client
offers `RuntimeTree::radial_stop_count` and `RuntimeTree::radial_stop`.
