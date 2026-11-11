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
    RawVirtualTable, TabIndicatorSide, TableColumn,
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

mod actions;
mod application;
mod avatar;
mod button;
mod calendar;
mod carousel;
mod command_palette;
mod content;
mod context_menu;
mod controls;
mod dataview;
mod dialog;
mod disclosure;
mod feedback;
mod file_prompt;
mod grid_layout;
mod inputs;
mod navigation;
mod number;
mod otp;
mod pickers;
mod rating;
mod resizable;
mod scroll;
mod selection;
mod surfaces;
mod text;
mod titlebar;
mod toast;
mod tooltip;
mod typography;
mod workspace;

pub use actions::*;
pub use application::*;
pub use avatar::*;
pub use button::*;
pub use calendar::*;
pub use carousel::*;
pub use command_palette::*;
pub use content::*;
pub use context_menu::*;
pub use controls::*;
pub use dataview::*;
pub use dialog::*;
pub use disclosure::*;
pub use feedback::*;
pub use file_prompt::*;
pub use grid_layout::*;
pub use inputs::*;
pub use navigation::*;
pub use number::*;
pub use otp::*;
pub use pickers::*;
pub use rating::*;
pub use resizable::*;
pub use scroll::*;
pub use selection::*;
pub use surfaces::*;
pub use text::*;
pub use titlebar::*;
pub use toast::*;
pub use tooltip::*;
pub use typography::*;
pub use workspace::*;
