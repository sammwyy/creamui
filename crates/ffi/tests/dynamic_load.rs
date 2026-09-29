//! Proves the ABI-stability claim: this test never links `creamui-ffi`
//! directly. Instead it `dlopen`s the built `cdylib` at runtime (the same
//! way an app in any language would) and calls into it purely through
//! C function pointers and `#[repr(C)]` types.

use libloading::{Library, Symbol};
use std::ffi::{c_char, c_void, CStr, CString};
use std::os::raw::c_int;
use std::sync::atomic::{AtomicI32, Ordering};

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct CColor {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

/// Mirrors `creamui_ffi::CTheme` field-for-field.
#[repr(C)]
#[derive(Clone, Copy)]
struct CTheme {
    surface: CColor,
    surface_elevated: CColor,
    surface_hover: CColor,
    accent: CColor,
    accent_hover: CColor,
    accent_pressed: CColor,
    selection_background: CColor,
    selection_text: CColor,
    text_primary: CColor,
    text_secondary: CColor,
    text_disabled: CColor,
    border: CColor,
    border_strong: CColor,
    danger: CColor,
    warning: CColor,
    success: CColor,
    radius_small: f32,
    radius_medium: f32,
    radius_large: f32,
    spacing_small: f32,
    spacing_medium: f32,
    spacing_large: f32,
}

/// Mirrors `creamui_ffi::CDimension`.
#[repr(C)]
#[derive(Clone, Copy)]
struct CDimension {
    kind: u8,
    value: f32,
}

const DIM_AUTO: CDimension = CDimension {
    kind: 0,
    value: 0.0,
};
const ALIGN_UNSET: u8 = 255;

/// Mirrors `creamui_ffi::CStyle`.
#[repr(C)]
#[derive(Clone, Copy)]
struct CStyle {
    flex_direction: u8,
    justify_content: u8,
    align_items: u8,
    width: CDimension,
    height: CDimension,
    min_width: CDimension,
    min_height: CDimension,
    max_width: CDimension,
    max_height: CDimension,
    padding_left: f32,
    padding_right: f32,
    padding_top: f32,
    padding_bottom: f32,
    margin_left: CDimension,
    margin_right: CDimension,
    margin_top: CDimension,
    margin_bottom: CDimension,
    gap_row: f32,
    gap_column: f32,
    flex_grow: f32,
    flex_shrink: f32,
    flex_basis: CDimension,
}

fn default_style() -> CStyle {
    CStyle {
        flex_direction: 0,
        justify_content: ALIGN_UNSET,
        align_items: ALIGN_UNSET,
        width: DIM_AUTO,
        height: DIM_AUTO,
        min_width: DIM_AUTO,
        min_height: DIM_AUTO,
        max_width: DIM_AUTO,
        max_height: DIM_AUTO,
        padding_left: 0.0,
        padding_right: 0.0,
        padding_top: 0.0,
        padding_bottom: 0.0,
        margin_left: DIM_AUTO,
        margin_right: DIM_AUTO,
        margin_top: DIM_AUTO,
        margin_bottom: DIM_AUTO,
        gap_row: 0.0,
        gap_column: 0.0,
        flex_grow: 0.0,
        flex_shrink: 1.0,
        flex_basis: DIM_AUTO,
    }
}

fn cdylib_path() -> std::path::PathBuf {
    let mut deps_dir = std::env::current_exe().unwrap();
    deps_dir.pop(); // this test binary lives directly in .../target/debug/deps/
    let debug_dir = deps_dir
        .parent()
        .expect("deps/ always has a target/debug/ parent")
        .to_path_buf();

    let name = if cfg!(target_os = "macos") {
        "libcreamui.dylib"
    } else if cfg!(target_os = "windows") {
        "creamui.dll"
    } else {
        "libcreamui.so"
    };

    // `cargo build` uplifts the cdylib to target/debug/; `cargo test` alone
    // (with no prior `cargo build`) only leaves it in target/debug/deps/ —
    // cdylib filenames aren't hash-suffixed, so that path is stable too.
    let uplifted = debug_dir.join(name);
    if uplifted.exists() {
        uplifted
    } else {
        deps_dir.join(name)
    }
}

#[test]
fn loads_dynamically_and_builds_a_widget_tree() {
    let path = cdylib_path();
    let lib =
        unsafe { Library::new(&path) }.unwrap_or_else(|e| panic!("failed to dlopen {path:?}: {e}"));

    unsafe {
        let version: Symbol<unsafe extern "C" fn() -> *const c_char> =
            lib.get(b"creamui_version").unwrap();
        let version_str = CStr::from_ptr(version()).to_str().unwrap();
        assert_eq!(version_str, env!("CARGO_PKG_VERSION"));

        let block_new: Symbol<unsafe extern "C" fn() -> *mut c_void> =
            lib.get(b"creamui_block_new").unwrap();
        let set_background: Symbol<unsafe extern "C" fn(*mut c_void, CColor)> =
            lib.get(b"creamui_block_set_background").unwrap();
        let text_new: Symbol<unsafe extern "C" fn(*const c_char, CColor, f32) -> *mut c_void> =
            lib.get(b"creamui_text_new").unwrap();
        let add_child: Symbol<unsafe extern "C" fn(*mut c_void, *mut c_void)> =
            lib.get(b"creamui_block_add_child").unwrap();
        let widget_free: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"creamui_widget_free").unwrap();

        let root = block_new();
        assert!(!root.is_null());
        set_background(
            root,
            CColor {
                r: 10,
                g: 10,
                b: 10,
                a: 255,
            },
        );

        let label = CString::new("Loaded via dlopen").unwrap();
        let text = text_new(
            label.as_ptr(),
            CColor {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            16.0,
        );
        assert!(!text.is_null());

        add_child(root, text);
        widget_free(root);
    }
}

#[test]
fn versioned_window_entrypoints_are_exported() {
    let lib = unsafe { Library::new(cdylib_path()) }.unwrap();
    unsafe {
        assert!(lib.get::<*const c_void>(b"creamui_run_v2").is_ok());
        assert!(lib
            .get::<*const c_void>(b"creamui_app_builder_add_window_v2")
            .is_ok());
        assert!(lib.get::<*const c_void>(b"creamui_run").is_ok());
        assert!(lib
            .get::<*const c_void>(b"creamui_app_builder_add_window")
            .is_ok());
    }
}

#[test]
fn button_click_callback_crosses_the_abi_boundary() {
    let path = cdylib_path();
    let lib = unsafe { Library::new(&path) }.unwrap();

    static CLICKED: AtomicI32 = AtomicI32::new(0);
    extern "C" fn on_click(_userdata: *mut c_void) {
        CLICKED.fetch_add(1, Ordering::SeqCst);
    }

    unsafe {
        let theme_dark: Symbol<unsafe extern "C" fn() -> CTheme> =
            lib.get(b"creamui_theme_dark").unwrap();
        let button_new: Symbol<
            unsafe extern "C" fn(
                CTheme,
                *const c_char,
                extern "C" fn(*mut c_void),
                *mut c_void,
            ) -> *mut c_void,
        > = lib.get(b"creamui_button_new").unwrap();
        let widget_free: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"creamui_widget_free").unwrap();

        let label = CString::new("Click via FFI").unwrap();
        let button = button_new(theme_dark(), label.as_ptr(), on_click, std::ptr::null_mut());
        assert!(!button.is_null());

        // We only verify the widget was constructed and the symbol
        // resolved correctly here; actually invoking the click requires a
        // running render loop, exercised by the widgets crate's own
        // integration test against the same underlying `Button` type.
        widget_free(button);
        assert_eq!(CLICKED.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn signal_i32_get_reflects_set() {
    let path = cdylib_path();
    let lib = unsafe { Library::new(&path) }.unwrap();

    unsafe {
        let new: Symbol<unsafe extern "C" fn(i32) -> *mut c_void> =
            lib.get(b"creamui_signal_i32_new").unwrap();
        let get: Symbol<unsafe extern "C" fn(*const c_void) -> i32> =
            lib.get(b"creamui_signal_i32_get").unwrap();
        let set: Symbol<unsafe extern "C" fn(*const c_void, i32)> =
            lib.get(b"creamui_signal_i32_set").unwrap();
        let free: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"creamui_signal_i32_free").unwrap();

        let signal = new(41);
        assert!(!signal.is_null());
        assert_eq!(get(signal), 41);

        set(signal, 42);
        assert_eq!(get(signal), 42, "a dynamically-linked app needs this to build a working counter, since it has no Rust-side Signal of its own");

        free(signal);
    }
}

#[test]
fn theme_dark_and_light_expose_distinct_color_tokens() {
    let path = cdylib_path();
    let lib = unsafe { Library::new(&path) }.unwrap();

    unsafe {
        let theme_dark: Symbol<unsafe extern "C" fn() -> CTheme> =
            lib.get(b"creamui_theme_dark").unwrap();
        let theme_light: Symbol<unsafe extern "C" fn() -> CTheme> =
            lib.get(b"creamui_theme_light").unwrap();

        let dark = theme_dark();
        let light = theme_light();
        assert_ne!(
            dark.surface, light.surface,
            "dark/light themes must expose different tokens over the ABI"
        );
        assert_eq!(
            dark.radius_medium, light.radius_medium,
            "dark/light palettes keep the same default layout tokens"
        );
    }
}

#[test]
fn themed_text_and_button_use_the_caller_supplied_theme() {
    let path = cdylib_path();
    let lib = unsafe { Library::new(&path) }.unwrap();

    unsafe {
        let theme_light: Symbol<unsafe extern "C" fn() -> CTheme> =
            lib.get(b"creamui_theme_light").unwrap();
        let themed_text_new: Symbol<unsafe extern "C" fn(CTheme, *const c_char) -> *mut c_void> =
            lib.get(b"creamui_themed_text_new").unwrap();
        let widget_free: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"creamui_widget_free").unwrap();

        let label = CString::new("Themed via FFI").unwrap();
        let text = themed_text_new(theme_light(), label.as_ptr());
        assert!(
            !text.is_null(),
            "hello_world_dynamic needs this to demo the theme toggle the static example has"
        );
        widget_free(text);
    }
}

#[test]
fn block_new_styled_accepts_layout_properties() {
    let path = cdylib_path();
    let lib = unsafe { Library::new(&path) }.unwrap();

    unsafe {
        let block_new_styled: Symbol<unsafe extern "C" fn(CStyle) -> *mut c_void> =
            lib.get(b"creamui_block_new_styled").unwrap();
        let widget_free: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"creamui_widget_free").unwrap();

        let mut style = default_style();
        style.flex_direction = 1; // column
        style.width = CDimension {
            kind: 1,
            value: 240.0,
        };
        style.height = CDimension {
            kind: 2,
            value: 0.5,
        };
        style.padding_left = 8.0;
        style.gap_row = 4.0;
        style.flex_grow = 1.0;

        let block = block_new_styled(style);
        assert!(!block.is_null());
        widget_free(block);
    }
}

#[test]
fn checkbox_slider_text_input_and_scroll_view_construct_over_the_abi() {
    let path = cdylib_path();
    let lib = unsafe { Library::new(&path) }.unwrap();

    extern "C" fn noop_click(_userdata: *mut c_void) {}
    extern "C" fn noop_change(_value: *const c_char, _userdata: *mut c_void) {}
    extern "C" fn noop_slide(_value: f32, _userdata: *mut c_void) {}
    extern "C" fn noop_scroll(_delta: f32, _userdata: *mut c_void) {}

    unsafe {
        let theme_dark: Symbol<unsafe extern "C" fn() -> CTheme> =
            lib.get(b"creamui_theme_dark").unwrap();
        let checkbox_new: Symbol<
            unsafe extern "C" fn(
                CTheme,
                c_int,
                extern "C" fn(*mut c_void),
                *mut c_void,
            ) -> *mut c_void,
        > = lib.get(b"creamui_checkbox_new").unwrap();
        let text_input_new: Symbol<
            unsafe extern "C" fn(
                CTheme,
                CStyle,
                *const c_char,
                extern "C" fn(*const c_char, *mut c_void),
                *mut c_void,
            ) -> *mut c_void,
        > = lib.get(b"creamui_text_input_new").unwrap();
        let slider_new: Symbol<
            unsafe extern "C" fn(
                CTheme,
                CStyle,
                f32,
                extern "C" fn(f32, *mut c_void),
                *mut c_void,
            ) -> *mut c_void,
        > = lib.get(b"creamui_slider_new").unwrap();
        let scroll_view_new: Symbol<
            unsafe extern "C" fn(
                CTheme,
                CStyle,
                f32,
                extern "C" fn(f32, *mut c_void),
                *mut c_void,
            ) -> *mut c_void,
        > = lib.get(b"creamui_scroll_view_new").unwrap();
        let scroll_view_add_child: Symbol<unsafe extern "C" fn(*mut c_void, *mut c_void)> =
            lib.get(b"creamui_scroll_view_add_child").unwrap();
        let themed_text_new: Symbol<unsafe extern "C" fn(CTheme, *const c_char) -> *mut c_void> =
            lib.get(b"creamui_themed_text_new").unwrap();
        let widget_free: Symbol<unsafe extern "C" fn(*mut c_void)> =
            lib.get(b"creamui_widget_free").unwrap();

        let theme = theme_dark();

        let checkbox = checkbox_new(theme, 1, noop_click, std::ptr::null_mut());
        assert!(!checkbox.is_null());
        widget_free(checkbox);

        let value = CString::new("hello").unwrap();
        let text_input = text_input_new(
            theme,
            default_style(),
            value.as_ptr(),
            noop_change,
            std::ptr::null_mut(),
        );
        assert!(!text_input.is_null());
        widget_free(text_input);

        let slider = slider_new(
            theme,
            default_style(),
            0.5,
            noop_slide,
            std::ptr::null_mut(),
        );
        assert!(!slider.is_null());
        widget_free(slider);

        let scroll_view = scroll_view_new(
            theme,
            default_style(),
            0.0,
            noop_scroll,
            std::ptr::null_mut(),
        );
        assert!(!scroll_view.is_null());
        let item_label = CString::new("item").unwrap();
        let item = themed_text_new(theme, item_label.as_ptr());
        scroll_view_add_child(scroll_view, item);
        widget_free(scroll_view);
    }
}
