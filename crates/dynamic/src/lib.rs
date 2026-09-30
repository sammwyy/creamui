//! Safe, dependency-light client for `dlopen`-ing the `creamui` `cdylib`
//! and driving it through its C ABI (`creamui-ffi`).
//!
//! An app that talks to the ABI directly (like `examples/hello_world_dynamic`
//! used to) has to hand-declare every `#[repr(C)]` struct, every
//! `extern "C" fn` pointer type, and every `dlopen`/`dlsym` call itself —
//! and get it byte-for-byte right, since a mismatch is a silent ABI
//! violation, not a compile error. This crate does that once: it resolves
//! every symbol eagerly in [`Runtime::load`] (so a missing/renamed symbol
//! fails loudly, at startup, instead of as a crash the first time some
//! unrelated widget is used), and wraps the result in a safe, ergonomic API
//! that mirrors the native `creamui-core`/`creamui-widgets`/`creamui-render`
//! crates closely enough that porting code between "statically linked" and
//! "dlopen'd" is mostly a search-and-replace.
//!
//! It depends only on `creamui-abi` (the plain `#[repr(C)]` types shared
//! with `creamui-ffi`, with no engine dependency of its own) and
//! `libloading` — an app using this crate stays as free of the CreamUI
//! engine at compile time as one talking to the C ABI by hand would be.
//!
//! ```no_run
//! use creamui_dynamic::{block_styled, button, themed_text, Runtime, SignalI32, Style, WindowOptions};
//!
//! let rt = Runtime::load_default();
//! let count = SignalI32::new(&rt, 0);
//!
//! creamui_dynamic::run(&rt, WindowOptions::default(), rt.theme_dark().surface, |_handle| {}, {
//!     let count = count.clone();
//!     move |ctx: &creamui_dynamic::Context, size| {
//!         let theme = ctx.runtime().theme_dark();
//!         let count_for_click = count.clone();
//!         block_styled(ctx, Style { width: creamui_dynamic::Dimension::length(size.width), ..Style::default() })
//!             .child(themed_text(ctx, theme, &format!("Clicked {} times", count.get())))
//!             .child(button(ctx, theme, "Click me", move || count_for_click.set(count_for_click.get() + 1)))
//!     }
//! });
//! ```

mod arena;
mod runtime;
mod signal;
mod value;
mod widget;
mod window;

pub use creamui_abi::{
    CColorScheme, CNode, CPaintOp, CRadialStop, CRect, CUI_NODE_KIND_CONTAINER, CUI_NODE_KIND_TEXT,
    CUI_NODE_NONE,
};
pub use runtime::{LoadError, Runtime, RuntimeTree};
pub use signal::{SignalF32, SignalI32, SignalString};
pub use value::{BlurRegion, Color, Dimension, RenderBackend, Size, Style, Theme, WindowOptions};
pub use value::{
    ALIGN_BASELINE, ALIGN_CENTER, ALIGN_END, ALIGN_FLEX_END, ALIGN_FLEX_START, ALIGN_START,
    ALIGN_STRETCH, ALIGN_UNSET, FLEX_DIRECTION_COLUMN, FLEX_DIRECTION_COLUMN_REVERSE,
    FLEX_DIRECTION_ROW, FLEX_DIRECTION_ROW_REVERSE, JUSTIFY_CENTER, JUSTIFY_END, JUSTIFY_FLEX_END,
    JUSTIFY_FLEX_START, JUSTIFY_SPACE_AROUND, JUSTIFY_SPACE_BETWEEN, JUSTIFY_SPACE_EVENLY,
    JUSTIFY_START, JUSTIFY_STRETCH,
};
pub use widget::{
    block, block_styled, button, checkbox, scroll_view, slider, text, text_area, text_input,
    themed_text, themed_text_secondary, themed_text_sized, Widget,
};
pub use window::{run, AppBuilder, Context, WindowHandle};
