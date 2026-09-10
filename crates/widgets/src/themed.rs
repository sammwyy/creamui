//! Themed widgets: opinionated, styled wrappers around the headless widgets
//! in [`crate::raw`]. Each one reads its appearance via `use_theme()` at
//! construction time — see `creamui_reactive::with_context_scope` — so it
//! always reflects the window's current theme, including runtime switches.
//!
//! These are meant to be copied and adapted: a themed `Button` is nothing
//! more than a [`crate::raw::RawButton`] with theme-derived style baked in,
//! so writing a derived component (e.g. a `DangerButton`) is just writing a
//! new constructor function in the same shape.

use crate::raw::{
    RawButton, RawCheckbox, RawListView, RawScrollView, RawSidebar, RawSlider, RawSpinner,
    RawSwitch, RawTab, RawTable, RawTabs, RawText, RawTextArea, RawTextInput, RawView,
    TabIndicatorSide, TableColumn,
};
use creamui_core::layout::{
    AlignItems, Dimension, JustifyContent, LengthPercentage, Rect as LayoutRect, Style,
};
use creamui_core::{
    BoxedWidget, CursorIcon, KeyInput, Painter, Point, Rect, Styled, TextAlign, Widget,
};
use creamui_theme::{use_theme, Color, SelectionStyle, Theme};
use std::rc::Rc;

fn centered_box_style(padding: f32) -> Style {
    Style {
        padding: LayoutRect {
            left: LengthPercentage::Length(padding),
            right: LengthPercentage::Length(padding),
            top: LengthPercentage::Length(padding * 0.6),
            bottom: LengthPercentage::Length(padding * 0.6),
        },
        justify_content: Some(JustifyContent::Center),
        align_items: Some(AlignItems::Center),
        ..Default::default()
    }
}

mod avatar;
mod button;
mod controls;
mod dataview;
mod feedback;
mod inputs;
mod navigation;
mod pickers;
mod scroll;
mod selection;
mod surfaces;
mod text;
mod typography;

pub use avatar::*;
pub use button::*;
pub use controls::*;
pub use dataview::*;
pub use feedback::*;
pub use inputs::*;
pub use navigation::*;
pub use pickers::*;
pub use scroll::*;
pub use selection::*;
pub use surfaces::*;
pub use text::*;
pub use typography::*;
