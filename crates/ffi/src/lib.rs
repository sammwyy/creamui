//! ABI-stable C interface for consuming CreamUI as a shared library.
//!
//! This crate builds as a `cdylib`: an app can `dlopen`/link it and the
//! rest of the CreamUI engine (reactivity, layout, rendering) stays inside
//! the shared library, so multiple apps on a system can share one runtime
//! instead of statically bundling their own copy. Everything crossing the
//! boundary is either a plain `#[repr(C)]` value or an opaque pointer
//! (`*mut CWidget`) — never a Rust trait object or generic type — which is
//! what keeps the layout stable across compiler/library versions.
//!
//! All entry points are `extern "C"` and `#[no_mangle]`. Pointers returned
//! by `_new` functions are owned by the caller and must eventually be
//! passed to exactly one consuming call: [`creamui_block_add_child`] /
//! [`creamui_scroll_view_add_child`] (which take ownership of the child) or
//! [`creamui_run`] (which takes ownership of the root), or else freed with
//! [`creamui_widget_free`].

use creamui_abi::{DIMENSION_LENGTH, DIMENSION_PERCENT};
use creamui_core::layout::{
    AlignItems, Dimension, FlexDirection, JustifyContent, LengthPercentage, LengthPercentageAuto,
    Rect as LayoutRect, Size as LayoutSize, Style,
};
use creamui_core::{BoxedWidget, Size, Styled};
use creamui_reactive::Signal;
use creamui_render::{AppBuilder as RenderAppBuilder, WindowHandle};
use creamui_theme::{Color, ColorScheme, Theme};
use creamui_widgets::raw::{RawText, RawView};
use creamui_widgets::themed::{
    Button as ThemedButton, Checkbox as ThemedCheckbox, ScrollView as ThemedScrollView,
    Slider as ThemedSlider, Text as ThemedText, TextArea as ThemedTextArea,
    TextInput as ThemedTextInput,
};
use std::ffi::{c_char, c_void, CStr, CString};
use std::os::raw::c_int;

// Plain `#[repr(C)]` value types (colors, theme tokens, style, window
// options) live in `creamui-abi`, shared verbatim with `creamui-dynamic` on
// the consuming side of this ABI so the two can never drift out of sync.
// Re-exported here so existing code importing them from `creamui_ffi`
// (this crate's public name) keeps working unchanged.
pub use creamui_abi::{CColor, CDimension, CStyle, CTheme, CWindowOptions};
pub use creamui_abi::{CUI_RENDER_BACKEND_CPU, CUI_RENDER_BACKEND_GPU};

/// Opaque handle to a reactive `i32` value.
///
/// Reading it (via [`creamui_signal_i32_get`]) while building a widget tree
/// inside a [`creamui_run`] `build` callback subscribes that render to
/// future writes, exactly like a native Rust `creamui_reactive::Signal` —
/// this is what lets a C click handler trigger a re-render.
pub struct CSignalI32(Signal<i32>);

/// Creates a reactive `i32` signal with an initial value.
#[no_mangle]
pub extern "C" fn creamui_signal_i32_new(initial: i32) -> *mut CSignalI32 {
    Box::into_raw(Box::new(CSignalI32(Signal::new(initial))))
}

/// Reads the current value, subscribing the enclosing render (if any) to
/// future [`creamui_signal_i32_set`] calls.
///
/// # Safety
/// `signal` must be a valid, non-null pointer from [`creamui_signal_i32_new`]
/// that has not been freed.
#[no_mangle]
pub unsafe extern "C" fn creamui_signal_i32_get(signal: *const CSignalI32) -> i32 {
    (*signal).0.get()
}

/// Writes a new value, triggering a reactive re-render in anything that
/// previously read this signal via [`creamui_signal_i32_get`].
///
/// # Safety
/// `signal` must be a valid, non-null pointer from [`creamui_signal_i32_new`]
/// that has not been freed.
#[no_mangle]
pub unsafe extern "C" fn creamui_signal_i32_set(signal: *const CSignalI32, value: i32) {
    (*signal).0.set(value);
}

/// Frees a signal created with [`creamui_signal_i32_new`].
///
/// # Safety
/// `signal` must be a valid, non-null, not-yet-freed pointer from
/// [`creamui_signal_i32_new`], and must outlive every [`creamui_run`] call
/// that might still read or write it (typically: free it only after
/// `creamui_run` returns).
#[no_mangle]
pub unsafe extern "C" fn creamui_signal_i32_free(signal: *mut CSignalI32) {
    if !signal.is_null() {
        drop(Box::from_raw(signal));
    }
}

/// Opaque handle to a reactive `f32` value. Same semantics as
/// [`CSignalI32`], for widgets like [`creamui_slider_new`] and
/// [`creamui_scroll_view_new`] that read/write floats.
pub struct CSignalF32(Signal<f32>);

/// Creates a reactive `f32` signal with an initial value.
#[no_mangle]
pub extern "C" fn creamui_signal_f32_new(initial: f32) -> *mut CSignalF32 {
    Box::into_raw(Box::new(CSignalF32(Signal::new(initial))))
}

/// Reads the current value, subscribing the enclosing render (if any) to
/// future [`creamui_signal_f32_set`] calls.
///
/// # Safety
/// `signal` must be a valid, non-null pointer from [`creamui_signal_f32_new`]
/// that has not been freed.
#[no_mangle]
pub unsafe extern "C" fn creamui_signal_f32_get(signal: *const CSignalF32) -> f32 {
    (*signal).0.get()
}

/// Writes a new value, triggering a reactive re-render in anything that
/// previously read this signal via [`creamui_signal_f32_get`].
///
/// # Safety
/// `signal` must be a valid, non-null pointer from [`creamui_signal_f32_new`]
/// that has not been freed.
#[no_mangle]
pub unsafe extern "C" fn creamui_signal_f32_set(signal: *const CSignalF32, value: f32) {
    (*signal).0.set(value);
}

/// Frees a signal created with [`creamui_signal_f32_new`].
///
/// # Safety
/// Same contract as [`creamui_signal_i32_free`], for a [`creamui_signal_f32_new`]
/// pointer.
#[no_mangle]
pub unsafe extern "C" fn creamui_signal_f32_free(signal: *mut CSignalF32) {
    if !signal.is_null() {
        drop(Box::from_raw(signal));
    }
}

/// Opaque handle to a reactive `String` value, e.g. for a
/// [`creamui_text_input_new`]'s current value. Same "read subscribes, write
/// triggers a re-render" semantics as [`CSignalI32`].
pub struct CSignalString {
    signal: Signal<String>,
    /// Backs the pointer [`creamui_signal_string_get`] returns — owned here
    /// so it stays valid until the next call on this signal, rather than
    /// dangling the instant a temporary `CString` would otherwise drop.
    cache: std::cell::RefCell<CString>,
}

/// Creates a reactive `String` signal with an initial value.
///
/// # Safety
/// `initial` must be a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn creamui_signal_string_new(initial: *const c_char) -> *mut CSignalString {
    let initial = cstr_to_string(initial);
    let cache = CString::new(initial.clone()).unwrap_or_default();
    Box::into_raw(Box::new(CSignalString {
        signal: Signal::new(initial),
        cache: std::cell::RefCell::new(cache),
    }))
}

/// Reads the current value, subscribing the enclosing render (if any) to
/// future [`creamui_signal_string_set`] calls. The returned pointer is valid
/// only until the next call to [`creamui_signal_string_get`],
/// [`creamui_signal_string_set`], or [`creamui_signal_string_free`] on this
/// same signal — copy it out before then if it needs to outlive that.
///
/// # Safety
/// `signal` must be a valid, non-null pointer from
/// [`creamui_signal_string_new`] that has not been freed.
#[no_mangle]
pub unsafe extern "C" fn creamui_signal_string_get(signal: *const CSignalString) -> *const c_char {
    let signal = &*signal;
    let value = signal.signal.get();
    let mut cache = signal.cache.borrow_mut();
    *cache = CString::new(value).unwrap_or_default();
    cache.as_ptr()
}

/// Writes a new value, triggering a reactive re-render in anything that
/// previously read this signal via [`creamui_signal_string_get`].
///
/// # Safety
/// `signal` must be a valid, non-null pointer from
/// [`creamui_signal_string_new`] that has not been freed. `value` must be a
/// valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn creamui_signal_string_set(
    signal: *const CSignalString,
    value: *const c_char,
) {
    (*signal).signal.set(cstr_to_string(value));
}

/// Frees a signal created with [`creamui_signal_string_new`].
///
/// # Safety
/// Same contract as [`creamui_signal_i32_free`], for a
/// [`creamui_signal_string_new`] pointer.
#[no_mangle]
pub unsafe extern "C" fn creamui_signal_string_free(signal: *mut CSignalString) {
    if !signal.is_null() {
        drop(Box::from_raw(signal));
    }
}

// `CColor`/`CTheme`/`CStyle` are foreign types now (they live in
// `creamui-abi`, shared with `creamui-dynamic`), and so are `Color`/`Theme`/
// `Style` (from `creamui-theme`/`creamui-core`) — the orphan rule forbids
// `impl From<Foreign> for OtherForeign`, so these are plain conversion
// functions instead of trait impls.
fn color_from_c(c: CColor) -> Color {
    Color::rgba(c.r, c.g, c.b, c.a)
}

fn color_to_c(c: Color) -> CColor {
    CColor {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

fn theme_to_c(t: Theme) -> CTheme {
    CTheme {
        surface: color_to_c(t.surface),
        surface_elevated: color_to_c(t.surface_elevated),
        surface_hover: color_to_c(t.surface_hover),
        accent: color_to_c(t.accent),
        accent_hover: color_to_c(t.accent_hover),
        accent_pressed: color_to_c(t.accent_pressed),
        selection_background: color_to_c(t.selection_background),
        selection_text: color_to_c(t.selection_text),
        text_primary: color_to_c(t.text_primary),
        text_secondary: color_to_c(t.text_secondary),
        text_disabled: color_to_c(t.text_disabled),
        border: color_to_c(t.border),
        border_strong: color_to_c(t.border_strong),
        danger: color_to_c(t.danger),
        warning: color_to_c(t.warning),
        success: color_to_c(t.success),
        radius_small: t.radius_small,
        radius_medium: t.radius_medium,
        radius_large: t.radius_large,
        spacing_small: t.spacing_small,
        spacing_medium: t.spacing_medium,
        spacing_large: t.spacing_large,
    }
}

fn theme_from_c(t: CTheme) -> Theme {
    let colors = ColorScheme {
        surface: color_from_c(t.surface),
        surface_elevated: color_from_c(t.surface_elevated),
        surface_hover: color_from_c(t.surface_hover),
        accent: color_from_c(t.accent),
        accent_hover: color_from_c(t.accent_hover),
        accent_pressed: color_from_c(t.accent_pressed),
        selection_background: color_from_c(t.selection_background),
        selection_text: color_from_c(t.selection_text),
        text_primary: color_from_c(t.text_primary),
        text_secondary: color_from_c(t.text_secondary),
        text_disabled: color_from_c(t.text_disabled),
        border: color_from_c(t.border),
        border_strong: color_from_c(t.border_strong),
        danger: color_from_c(t.danger),
        warning: color_from_c(t.warning),
        success: color_from_c(t.success),
    };
    Theme::default().with_colors(colors)
}

/// Runs `f` inside a context scope providing `theme`, so the themed widget
/// constructors it calls (which read `use_theme()`, not an explicit
/// parameter) resolve it correctly. C callers construct one widget per call
/// rather than a whole tree inside `build_ui`, so each such FFI function
/// opens its own short-lived scope instead of relying on one already being
/// active.
fn with_theme_scope<R>(theme: Theme, f: impl FnOnce() -> R) -> R {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        f()
    })
}

/// Returns the bundled default dark theme's tokens.
#[no_mangle]
pub extern "C" fn creamui_theme_dark() -> CTheme {
    theme_to_c(Theme::dark())
}

/// Returns the bundled default light theme's tokens.
#[no_mangle]
pub extern "C" fn creamui_theme_light() -> CTheme {
    theme_to_c(Theme::light())
}

// `CDimension` (like `CColor`/`CTheme`/`CStyle`/`CWindowOptions`) lives in
// `creamui-abi` now, shared with `creamui-dynamic` — these are free
// functions rather than an inherent `impl CDimension` block since the
// orphan rule forbids inherent impls on a foreign type.
fn dimension_to_dimension(d: CDimension) -> Dimension {
    match d.kind {
        DIMENSION_LENGTH => Dimension::Length(d.value),
        DIMENSION_PERCENT => Dimension::Percent(d.value),
        _ => Dimension::Auto,
    }
}

fn dimension_to_length_percentage(d: CDimension) -> LengthPercentage {
    match d.kind {
        DIMENSION_PERCENT => LengthPercentage::Percent(d.value),
        _ => LengthPercentage::Length(d.value),
    }
}

fn dimension_to_length_percentage_auto(d: CDimension) -> LengthPercentageAuto {
    match d.kind {
        DIMENSION_LENGTH => LengthPercentageAuto::Length(d.value),
        DIMENSION_PERCENT => LengthPercentageAuto::Percent(d.value),
        _ => LengthPercentageAuto::Auto,
    }
}

fn decode_justify_content(code: u8) -> Option<JustifyContent> {
    Some(match code {
        0 => JustifyContent::Start,
        1 => JustifyContent::End,
        2 => JustifyContent::FlexStart,
        3 => JustifyContent::FlexEnd,
        4 => JustifyContent::Center,
        5 => JustifyContent::Stretch,
        6 => JustifyContent::SpaceBetween,
        7 => JustifyContent::SpaceAround,
        8 => JustifyContent::SpaceEvenly,
        _ => return None,
    })
}

fn decode_align_items(code: u8) -> Option<AlignItems> {
    Some(match code {
        0 => AlignItems::Start,
        1 => AlignItems::End,
        2 => AlignItems::FlexStart,
        3 => AlignItems::FlexEnd,
        4 => AlignItems::Center,
        5 => AlignItems::Stretch,
        9 => AlignItems::Baseline,
        _ => return None,
    })
}

fn decode_flex_direction(code: u8) -> FlexDirection {
    match code {
        1 => FlexDirection::Column,
        2 => FlexDirection::RowReverse,
        3 => FlexDirection::ColumnReverse,
        _ => FlexDirection::Row,
    }
}

/// Returns a [`CStyle`] matching `Style::default()`. Thin wrapper over
/// [`CStyle::default_style`] (from `creamui-abi`) for parity with the rest
/// of this crate's `_new`/`_default`-style entry points.
#[no_mangle]
pub extern "C" fn creamui_style_default() -> CStyle {
    CStyle::default_style()
}

fn style_from_c(s: CStyle) -> Style {
    Style {
        display: creamui_core::layout::Display::Flex,
        flex_direction: decode_flex_direction(s.flex_direction),
        justify_content: decode_justify_content(s.justify_content),
        align_items: decode_align_items(s.align_items),
        size: LayoutSize {
            width: dimension_to_dimension(s.width),
            height: dimension_to_dimension(s.height),
        },
        min_size: LayoutSize {
            width: dimension_to_dimension(s.min_width),
            height: dimension_to_dimension(s.min_height),
        },
        max_size: LayoutSize {
            width: dimension_to_dimension(s.max_width),
            height: dimension_to_dimension(s.max_height),
        },
        padding: LayoutRect {
            left: LengthPercentage::Length(s.padding_left),
            right: LengthPercentage::Length(s.padding_right),
            top: LengthPercentage::Length(s.padding_top),
            bottom: LengthPercentage::Length(s.padding_bottom),
        },
        margin: LayoutRect {
            left: dimension_to_length_percentage_auto(s.margin_left),
            right: dimension_to_length_percentage_auto(s.margin_right),
            top: dimension_to_length_percentage_auto(s.margin_top),
            bottom: dimension_to_length_percentage_auto(s.margin_bottom),
        },
        gap: LayoutSize {
            width: dimension_to_length_percentage(CDimension::length(s.gap_column)),
            height: dimension_to_length_percentage(CDimension::length(s.gap_row)),
        },
        flex_grow: s.flex_grow,
        flex_shrink: s.flex_shrink,
        flex_basis: dimension_to_dimension(s.flex_basis),
        ..Default::default()
    }
}

enum WidgetKind {
    Block(RawView),
    Text(RawText),
    ThemedText(ThemedText),
    ThemedButton(ThemedButton),
    ThemedCheckbox(ThemedCheckbox),
    ThemedTextInput(ThemedTextInput),
    ThemedTextArea(ThemedTextArea),
    ThemedSlider(ThemedSlider),
    ThemedScrollView(ThemedScrollView),
}

impl WidgetKind {
    fn into_boxed(self) -> BoxedWidget {
        match self {
            WidgetKind::Block(w) => Box::new(w),
            WidgetKind::Text(w) => Box::new(w),
            WidgetKind::ThemedText(w) => Box::new(w),
            WidgetKind::ThemedButton(w) => Box::new(w),
            WidgetKind::ThemedCheckbox(w) => Box::new(w),
            WidgetKind::ThemedTextInput(w) => Box::new(w),
            WidgetKind::ThemedTextArea(w) => Box::new(w),
            WidgetKind::ThemedSlider(w) => Box::new(w),
            WidgetKind::ThemedScrollView(w) => Box::new(w),
        }
    }
}

/// Opaque handle to a not-yet-attached widget subtree.
pub struct CWidget(WidgetKind);

unsafe fn cstr_to_string(s: *const c_char) -> String {
    if s.is_null() {
        return String::new();
    }
    CStr::from_ptr(s).to_string_lossy().into_owned()
}

/// Returns the engine's version string (e.g. `"0.1.0"`), NUL-terminated,
/// valid for the lifetime of the process.
#[no_mangle]
pub extern "C" fn creamui_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

/// Creates a semantic block container that fills its parent.
#[no_mangle]
pub extern "C" fn creamui_block_new() -> *mut CWidget {
    let style = Style {
        display: creamui_core::layout::Display::Block,
        size: LayoutSize {
            width: Dimension::Percent(1.0),
            height: Dimension::Percent(1.0),
        },
        ..Default::default()
    };
    let widget = CWidget(WidgetKind::Block(RawView::new(style)));
    Box::into_raw(Box::new(widget))
}

/// Creates a plain container widget with a caller-supplied [`CStyle`],
/// giving full control over flex direction, sizing, padding, margin, gap,
/// and flex-grow/shrink/basis — the same style surface `creamui_widgets`'
/// `Block` exposes natively. The container always uses block layout.
#[no_mangle]
pub extern "C" fn creamui_block_new_styled(style: CStyle) -> *mut CWidget {
    let mut style = style_from_c(style);
    style.display = creamui_core::layout::Display::Block;
    let widget = CWidget(WidgetKind::Block(RawView::new(style)));
    Box::into_raw(Box::new(widget))
}

/// Sets a block's background color. `block` must be a live pointer from
/// [`creamui_block_new`]/[`creamui_block_new_styled`] that has not yet been
/// consumed.
///
/// # Safety
/// `block` must be a valid, non-null pointer returned by
/// [`creamui_block_new`]/[`creamui_block_new_styled`] and not yet passed to
/// [`creamui_block_add_child`], [`creamui_run`], or [`creamui_widget_free`].
#[no_mangle]
pub unsafe extern "C" fn creamui_block_set_background(block: *mut CWidget, color: CColor) {
    if block.is_null() {
        return;
    }
    if let WidgetKind::Block(v) = &mut (*block).0 {
        v.style.paint.background = Some(color_from_c(color).into());
    }
}

/// Sets a block's corner radius, in logical pixels.
///
/// # Safety
/// Same contract as [`creamui_block_set_background`].
#[no_mangle]
pub unsafe extern "C" fn creamui_block_set_corner_radius(block: *mut CWidget, radius: f32) {
    if block.is_null() {
        return;
    }
    if let WidgetKind::Block(v) = &mut (*block).0 {
        v.style.paint.corner_radius = Some(radius);
    }
}

/// Attaches `child` to `block`, taking ownership of `child` (it must not be
/// used or freed again after this call). Works for both
/// [`creamui_block_new`]/[`creamui_block_new_styled`] and
/// [`creamui_scroll_view_new`] parents.
///
/// # Safety
/// `block` and `child` must be valid, non-null, not-yet-consumed pointers
/// from this crate's `_new` functions, and must not alias each other.
#[no_mangle]
pub unsafe extern "C" fn creamui_block_add_child(block: *mut CWidget, child: *mut CWidget) {
    if block.is_null() || child.is_null() {
        return;
    }
    let child = *Box::from_raw(child);
    match &mut (*block).0 {
        WidgetKind::Block(v) => v.children.push(child.0.into_boxed()),
        WidgetKind::ThemedScrollView(v) => push_scroll_view_child(v, child.0.into_boxed()),
        _ => {}
    }
}

/// `ThemedScrollView`'s children live behind a consuming builder method
/// (`.child()`), not a public field — this takes ownership out of the `&mut`
/// reference via a throwaway placeholder so both `creamui_block_add_child`
/// and [`creamui_scroll_view_add_child`] can share this one code path.
fn push_scroll_view_child(view: &mut ThemedScrollView, child: BoxedWidget) {
    let placeholder = with_theme_scope(Theme::dark(), || {
        ThemedScrollView::new(Style::default(), 0.0, |_| {})
    });
    let taken = std::mem::replace(view, placeholder);
    *view = taken.child(child);
}

/// Attaches `child` to a scroll view created by [`creamui_scroll_view_new`],
/// taking ownership of `child`.
///
/// # Safety
/// Same contract as [`creamui_block_add_child`]; `view` must specifically be
/// a not-yet-consumed pointer from [`creamui_scroll_view_new`].
#[no_mangle]
pub unsafe extern "C" fn creamui_scroll_view_add_child(view: *mut CWidget, child: *mut CWidget) {
    if view.is_null() || child.is_null() {
        return;
    }
    let child = *Box::from_raw(child);
    if let WidgetKind::ThemedScrollView(v) = &mut (*view).0 {
        push_scroll_view_child(v, child.0.into_boxed());
    }
}

/// Creates an unthemed, single-line text label with an explicit color and
/// size.
///
/// # Safety
/// `text` must be a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn creamui_text_new(
    text: *const c_char,
    color: CColor,
    font_size: f32,
) -> *mut CWidget {
    let text = cstr_to_string(text);
    let widget = CWidget(WidgetKind::Text(RawText::new(
        text,
        color_from_c(color),
        font_size,
    )));
    Box::into_raw(Box::new(widget))
}

/// Creates a themed text label using `theme`'s primary text color (get one
/// from [`creamui_theme_dark`]/[`creamui_theme_light`]).
///
/// # Safety
/// `text` must be a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn creamui_themed_text_new(
    theme: CTheme,
    text: *const c_char,
) -> *mut CWidget {
    let text = cstr_to_string(text);
    let theme: Theme = theme_from_c(theme);
    let widget = CWidget(WidgetKind::ThemedText(with_theme_scope(theme, || {
        ThemedText::new(text)
    })));
    Box::into_raw(Box::new(widget))
}

/// Creates a themed text label using `theme`'s secondary (muted) text color
/// — e.g. for captions or de-emphasized helper text.
///
/// # Safety
/// `text` must be a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn creamui_themed_text_secondary_new(
    theme: CTheme,
    text: *const c_char,
) -> *mut CWidget {
    let text = cstr_to_string(text);
    let theme: Theme = theme_from_c(theme);
    let widget = CWidget(WidgetKind::ThemedText(with_theme_scope(theme, || {
        ThemedText::secondary(text)
    })));
    Box::into_raw(Box::new(widget))
}

/// Same as [`creamui_themed_text_new`], but with an explicit font size in
/// logical pixels instead of the default 14.0.
///
/// # Safety
/// `text` must be a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn creamui_themed_text_new_sized(
    theme: CTheme,
    text: *const c_char,
    font_size: f32,
) -> *mut CWidget {
    let text = cstr_to_string(text);
    let theme: Theme = theme_from_c(theme);
    let widget = CWidget(WidgetKind::ThemedText(with_theme_scope(theme, || {
        ThemedText::new(text).font_size(font_size)
    })));
    Box::into_raw(Box::new(widget))
}

/// Creates a themed button labeled `text`, invoking `on_click(userdata)` on
/// every click.
///
/// # Safety
/// `text` must be a valid NUL-terminated UTF-8 string. `on_click` must be
/// safe to call with `userdata` for as long as the returned widget (and any
/// tree it is attached to) is alive.
#[no_mangle]
pub unsafe extern "C" fn creamui_button_new(
    theme: CTheme,
    text: *const c_char,
    on_click: extern "C" fn(*mut c_void),
    userdata: *mut c_void,
) -> *mut CWidget {
    let text = cstr_to_string(text);
    // SAFETY contract above: the caller guarantees `userdata` stays valid
    // and `on_click` stays callable for as long as this widget tree lives.
    struct SendPtr(*mut c_void);
    unsafe impl Send for SendPtr {}
    let userdata = SendPtr(userdata);

    let theme: Theme = theme_from_c(theme);
    let button = with_theme_scope(theme, || {
        ThemedButton::new(text, move || {
            on_click(userdata.0);
        })
    });
    Box::into_raw(Box::new(CWidget(WidgetKind::ThemedButton(button))))
}

/// Creates a themed checkbox, invoking `on_click(userdata)` on every click
/// (same "caller owns the checked state" pattern as the native `Checkbox`:
/// toggle your own state in `on_click` and pass the new value back in on
/// the next `build` call).
///
/// # Safety
/// `on_click` must be safe to call with `userdata` for as long as the
/// returned widget (and any tree it is attached to) is alive.
#[no_mangle]
pub unsafe extern "C" fn creamui_checkbox_new(
    theme: CTheme,
    checked: c_int,
    on_click: extern "C" fn(*mut c_void),
    userdata: *mut c_void,
) -> *mut CWidget {
    struct SendPtr(*mut c_void);
    unsafe impl Send for SendPtr {}
    let userdata = SendPtr(userdata);

    let theme: Theme = theme_from_c(theme);
    let checkbox = with_theme_scope(theme, || {
        ThemedCheckbox::new(checked != 0, move || {
            on_click(userdata.0);
        })
    });
    Box::into_raw(Box::new(CWidget(WidgetKind::ThemedCheckbox(checkbox))))
}

/// Creates a themed single-line text input with a caller-supplied [`CStyle`]
/// (e.g. its width/height). `on_change(new_value, userdata)` fires on every
/// keystroke with a NUL-terminated UTF-8 string owned by the callee — valid
/// only for the duration of the call.
///
/// # Safety
/// `value` must be a valid NUL-terminated UTF-8 string. `on_change` must be
/// safe to call with `userdata` for as long as the returned widget (and any
/// tree it is attached to) is alive.
#[no_mangle]
pub unsafe extern "C" fn creamui_text_input_new(
    theme: CTheme,
    style: CStyle,
    value: *const c_char,
    on_change: extern "C" fn(*const c_char, *mut c_void),
    userdata: *mut c_void,
) -> *mut CWidget {
    struct SendPtr(*mut c_void);
    unsafe impl Send for SendPtr {}
    let userdata = SendPtr(userdata);

    let value = cstr_to_string(value);
    let theme_owned: Theme = theme_from_c(theme);
    let inner = with_theme_scope(theme_owned, || {
        ThemedTextInput::new(value, move |next: String| {
            // CString::new fails only on interior NULs, which a text input's
            // keystroke-built value can never contain (Key::Char never yields
            // '\0'), so this is infallible in practice.
            if let Ok(c_next) = CString::new(next) {
                on_change(c_next.as_ptr(), userdata.0);
            }
        })
        .layout(style_from_c(style))
    });
    Box::into_raw(Box::new(CWidget(WidgetKind::ThemedTextInput(inner))))
}

/// Sets the placeholder text (and its themed disabled-text color) shown
/// when a text input's value is empty.
///
/// # Safety
/// `input` must be a valid, non-null, not-yet-consumed pointer from
/// [`creamui_text_input_new`]. `text` must be a valid NUL-terminated UTF-8
/// string.
#[no_mangle]
pub unsafe extern "C" fn creamui_text_input_set_placeholder(
    theme: CTheme,
    input: *mut CWidget,
    text: *const c_char,
) {
    if input.is_null() {
        return;
    }
    let text = cstr_to_string(text);
    let theme: Theme = theme_from_c(theme);
    if let WidgetKind::ThemedTextInput(w) = &mut (*input).0 {
        with_theme_scope(theme, || {
            let taken = std::mem::replace(w, ThemedTextInput::new(String::new(), |_| {}));
            *w = taken.placeholder(text);
        });
    }
}

/// Creates a themed multi-line text area. Its ABI and controlled-value
/// callback contract mirror [`creamui_text_input_new`].
#[no_mangle]
pub unsafe extern "C" fn creamui_text_area_new(
    theme: CTheme,
    style: CStyle,
    value: *const c_char,
    on_change: extern "C" fn(*const c_char, *mut c_void),
    userdata: *mut c_void,
) -> *mut CWidget {
    struct SendPtr(*mut c_void);
    unsafe impl Send for SendPtr {}
    let userdata = SendPtr(userdata);
    let value = cstr_to_string(value);
    let theme_owned: Theme = theme_from_c(theme);
    let area = with_theme_scope(theme_owned, || {
        ThemedTextArea::new(value, move |next: String| {
            if let Ok(c_next) = CString::new(next) {
                on_change(c_next.as_ptr(), userdata.0);
            }
        })
        .layout(style_from_c(style))
    });
    Box::into_raw(Box::new(CWidget(WidgetKind::ThemedTextArea(area))))
}

/// Sets placeholder text on a text area created with
/// [`creamui_text_area_new`].
#[no_mangle]
pub unsafe extern "C" fn creamui_text_area_set_placeholder(
    theme: CTheme,
    input: *mut CWidget,
    text: *const c_char,
) {
    if input.is_null() {
        return;
    }
    let text = cstr_to_string(text);
    let theme: Theme = theme_from_c(theme);
    if let WidgetKind::ThemedTextArea(w) = &mut (*input).0 {
        with_theme_scope(theme, || {
            let taken = std::mem::replace(w, ThemedTextArea::new(String::new(), |_| {}));
            *w = taken.placeholder(text);
        });
    }
}

/// Sets the visual selection tokens on a text area. These values are kept on
/// the widget itself, so dynamic and statically linked applications use the
/// same renderer path.
///
/// # Safety
/// `input` must be a valid, live pointer returned by
/// [`creamui_text_area_new`].
#[no_mangle]
pub unsafe extern "C" fn creamui_text_area_set_selection_colors(
    input: *mut CWidget,
    background: CColor,
    text: CColor,
) {
    if input.is_null() {
        return;
    }
    if let WidgetKind::ThemedTextArea(w) = &mut (*input).0 {
        let placeholder =
            with_theme_scope(Theme::dark(), || ThemedTextArea::new(String::new(), |_| {}));
        let taken = std::mem::replace(w, placeholder);
        *w = taken
            .selection_background(color_from_c(background))
            .selection_text_color(color_from_c(text));
    }
}

/// Creates a themed horizontal slider with a caller-supplied [`CStyle`].
/// `on_change(value, userdata)` fires with the new `0.0..=1.0` value as the
/// handle is dragged.
///
/// # Safety
/// `on_change` must be safe to call with `userdata` for as long as the
/// returned widget (and any tree it is attached to) is alive.
#[no_mangle]
pub unsafe extern "C" fn creamui_slider_new(
    theme: CTheme,
    style: CStyle,
    value: f32,
    on_change: extern "C" fn(f32, *mut c_void),
    userdata: *mut c_void,
) -> *mut CWidget {
    struct SendPtr(*mut c_void);
    unsafe impl Send for SendPtr {}
    let userdata = SendPtr(userdata);

    let theme: Theme = theme_from_c(theme);
    let slider = with_theme_scope(theme, || {
        ThemedSlider::new(value, move |next| {
            on_change(next, userdata.0);
        })
        .layout(style_from_c(style))
    });
    Box::into_raw(Box::new(CWidget(WidgetKind::ThemedSlider(slider))))
}

/// Creates a themed vertically-scrollable container with a caller-supplied
/// [`CStyle`] (typically a fixed `width`/`height` viewport). Attach children
/// with [`creamui_scroll_view_add_child`]. `on_scroll(delta_y, userdata)`
/// fires on every wheel event over the view; the caller owns and clamps the
/// scroll offset, same as the native `ScrollView`.
///
/// # Safety
/// `on_scroll` must be safe to call with `userdata` for as long as the
/// returned widget (and any tree it is attached to) is alive.
#[no_mangle]
pub unsafe extern "C" fn creamui_scroll_view_new(
    theme: CTheme,
    style: CStyle,
    scroll_y: f32,
    on_scroll: extern "C" fn(f32, *mut c_void),
    userdata: *mut c_void,
) -> *mut CWidget {
    struct SendPtr(*mut c_void);
    unsafe impl Send for SendPtr {}
    let userdata = SendPtr(userdata);

    let theme: Theme = theme_from_c(theme);
    let scroll_view = with_theme_scope(theme, || {
        ThemedScrollView::new(style_from_c(style), scroll_y, move |delta| {
            on_scroll(delta, userdata.0);
        })
    });
    Box::into_raw(Box::new(CWidget(WidgetKind::ThemedScrollView(scroll_view))))
}

/// Frees a widget subtree that was never attached via
/// [`creamui_block_add_child`], [`creamui_scroll_view_add_child`], or
/// [`creamui_run`].
///
/// # Safety
/// `widget` must be a valid, non-null, not-yet-consumed pointer from one of
/// this crate's `_new` functions, and must not be used again afterward.
#[no_mangle]
pub unsafe extern "C" fn creamui_widget_free(widget: *mut CWidget) {
    if !widget.is_null() {
        drop(Box::from_raw(widget));
    }
}

/// Opaque handle for issuing window-level operations (resize, move,
/// always-on-top) from C, e.g. from a click handler. Valid for the lifetime
/// of the [`creamui_run`] call that produced it via `on_window_ready`; do
/// not use after `creamui_run` returns.
pub struct CWindowHandle(WindowHandle);

/// Requests a new logical-pixel window size.
///
/// # Safety
/// `handle` must be a valid, non-null pointer from a [`creamui_run`]
/// `on_window_ready` callback, still within that `creamui_run` call.
#[no_mangle]
pub unsafe extern "C" fn creamui_window_resize(
    handle: *const CWindowHandle,
    width: u32,
    height: u32,
) {
    if handle.is_null() {
        return;
    }
    (*handle).0.resize(width, height);
}

/// Moves the window's top-left corner to a logical-pixel screen position.
///
/// # Safety
/// Same contract as [`creamui_window_resize`].
#[no_mangle]
pub unsafe extern "C" fn creamui_window_set_position(handle: *const CWindowHandle, x: i32, y: i32) {
    if handle.is_null() {
        return;
    }
    (*handle).0.set_position(x, y);
}

/// Pins (`enabled != 0`) or unpins the window above all others.
///
/// # Safety
/// Same contract as [`creamui_window_resize`].
#[no_mangle]
pub unsafe extern "C" fn creamui_window_set_always_on_top(
    handle: *const CWindowHandle,
    enabled: c_int,
) {
    if handle.is_null() {
        return;
    }
    (*handle).0.set_always_on_top(enabled != 0);
}

/// Frees a handle obtained from a [`creamui_run`] `on_window_ready`
/// callback. Optional — the handle is also cleaned up when `creamui_run`
/// returns — but calling this lets an app stop holding onto it earlier.
///
/// # Safety
/// `handle` must be a valid, non-null, not-yet-freed pointer produced by a
/// `creamui_run` `on_window_ready` callback.
#[no_mangle]
pub unsafe extern "C" fn creamui_window_handle_free(handle: *mut CWindowHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

type CBuildFn = extern "C" fn(width: f32, height: f32, userdata: *mut c_void) -> *mut CWidget;
type CWindowReadyFn = extern "C" fn(handle: *mut CWindowHandle, userdata: *mut c_void);

fn window_options_from_c(options: CWindowOptions) -> creamui_render::WindowOptions {
    creamui_render::WindowOptions {
        title: unsafe { cstr_to_string(options.title) },
        width: options.width,
        height: options.height,
        position: None,
        resizable: options.resizable != 0,
        decorations: options.decorations != 0,
        transparent: options.transparent != 0,
        // The C ABI keeps its existing close semantics; the new app/tray
        // lifecycle controls are Rust-native for now.
        close_behavior: creamui_render::CloseBehavior::Close,
        backend: if options.backend == CUI_RENDER_BACKEND_CPU {
            creamui_render::RenderBackend::Cpu
        } else {
            creamui_render::RenderBackend::Gpu
        },
        theme: Theme::default(),
    }
}

/// Wraps a nullable [`CWindowReadyFn`] into the `FnOnce(WindowHandle)` closure
/// `creamui_render::run`/`AppBuilder::window` expect, shared by
/// [`creamui_run`] and [`creamui_app_builder_add_window`].
fn window_ready_callback(
    on_window_ready: Option<CWindowReadyFn>,
    userdata: *mut c_void,
) -> impl FnOnce(WindowHandle) {
    struct SendPtr(*mut c_void);
    unsafe impl Send for SendPtr {}
    let ready_userdata = SendPtr(userdata);
    move |handle: WindowHandle| {
        if let Some(on_ready) = on_window_ready {
            let boxed = Box::into_raw(Box::new(CWindowHandle(handle)));
            on_ready(boxed, ready_userdata.0);
        }
    }
}

/// Wraps a [`CBuildFn`] into the `Fn(Size) -> BoxedWidget` closure
/// `creamui_render::run`/`AppBuilder::window` expect, shared by
/// [`creamui_run`] and [`creamui_app_builder_add_window`].
fn build_callback(build: CBuildFn, userdata: *mut c_void) -> impl Fn(Size) -> BoxedWidget {
    struct SendPtr(*mut c_void);
    unsafe impl Send for SendPtr {}
    let build_userdata = SendPtr(userdata);
    move |size: Size| -> BoxedWidget {
        let raw = build(size.width, size.height, build_userdata.0);
        assert!(!raw.is_null(), "build callback returned a null widget");
        // SAFETY: `build` is contractually required to return an owned,
        // freshly-allocated widget pointer each call; we take ownership here.
        let widget = unsafe { *Box::from_raw(raw) };
        widget.0.into_boxed()
    }
}

/// Opens a window and runs the render loop until closed, calling `build`
/// once up front and again on every reactive change to construct the
/// widget tree for the current viewport size. If `on_window_ready` is
/// non-null, it is called exactly once, as soon as the window exists, with
/// a heap-allocated [`CWindowHandle`] the app can stash in its own
/// `userdata` and use later (e.g. from a click handler) to resize, move, or
/// pin the window — see [`creamui_window_resize`] and friends. Blocks until
/// the window is closed.
///
/// To open several windows sharing one process and event loop (e.g. a
/// desktop-shell dock), use [`creamui_app_builder_new`] instead.
///
/// # Safety
/// `options.title` must be a valid NUL-terminated UTF-8 string for the
/// duration of this call. `build` must return a valid, non-null pointer
/// from one of this crate's widget `_new` functions each time it is
/// called, and must be safe to call with `userdata` for the lifetime of
/// this call. `on_window_ready`, if non-null, must likewise be safe to call
/// with `userdata`.
#[no_mangle]
pub unsafe extern "C" fn creamui_run(
    options: CWindowOptions,
    background: CColor,
    build: CBuildFn,
    on_window_ready: Option<CWindowReadyFn>,
    userdata: *mut c_void,
) {
    creamui_render::run(
        window_options_from_c(options),
        color_from_c(background),
        window_ready_callback(on_window_ready, userdata),
        build_callback(build, userdata),
    );
}

/// Opaque builder for opening several windows sharing one process and one
/// event loop — e.g. a desktop-shell dock where each icon/panel is its own
/// window but spawning a process per icon would multiply fixed
/// per-process overhead (runtime, allocator, embedded font, and — for the
/// GPU backend — the graphics driver) for no benefit. Windows added via
/// [`creamui_app_builder_add_window`] keep fully independent reactive/paint
/// state; the process's windows stay open until every one of them has been
/// closed. See `creamui_render::AppBuilder` for the native Rust equivalent.
pub struct CAppBuilder(Option<RenderAppBuilder>);

/// Creates an empty builder. Add windows with
/// [`creamui_app_builder_add_window`], then open them all with
/// [`creamui_app_builder_run`].
#[no_mangle]
pub extern "C" fn creamui_app_builder_new() -> *mut CAppBuilder {
    Box::into_raw(Box::new(CAppBuilder(Some(RenderAppBuilder::new()))))
}

/// Queues a window to be opened when [`creamui_app_builder_run`] starts the
/// shared event loop. Arguments have the same meaning as the identically
/// named ones on [`creamui_run`].
///
/// # Safety
/// `builder` must be a valid, non-null, not-yet-`_run` pointer from
/// [`creamui_app_builder_new`]. `options.title` must be a valid
/// NUL-terminated UTF-8 string for the duration of this call. `build` and
/// `on_window_ready` have the same contract as on [`creamui_run`], scoped
/// to the eventual [`creamui_app_builder_run`] call instead.
#[no_mangle]
pub unsafe extern "C" fn creamui_app_builder_add_window(
    builder: *mut CAppBuilder,
    options: CWindowOptions,
    background: CColor,
    build: CBuildFn,
    on_window_ready: Option<CWindowReadyFn>,
    userdata: *mut c_void,
) {
    if builder.is_null() {
        return;
    }
    let slot = &mut (*builder).0;
    let owned = slot.take().expect(
        "creamui_app_builder_add_window: builder was already consumed by creamui_app_builder_run",
    );
    *slot = Some(owned.window(
        window_options_from_c(options),
        color_from_c(background),
        window_ready_callback(on_window_ready, userdata),
        build_callback(build, userdata),
    ));
}

/// Opens every window queued with [`creamui_app_builder_add_window`] and
/// runs one shared event loop until all of them have closed. Consumes and
/// frees `builder` — it must not be used again afterward.
///
/// # Safety
/// `builder` must be a valid, non-null, not-yet-`_run` pointer from
/// [`creamui_app_builder_new`].
#[no_mangle]
pub unsafe extern "C" fn creamui_app_builder_run(builder: *mut CAppBuilder) {
    if builder.is_null() {
        return;
    }
    let boxed = Box::from_raw(builder);
    let owned = boxed
        .0
        .expect("creamui_app_builder_run: builder was already consumed by an earlier creamui_app_builder_run");
    owned.run();
}
