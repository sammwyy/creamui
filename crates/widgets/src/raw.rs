//! Headless widgets: fully unstyled building blocks with no opinion on
//! color, radius, or spacing. Themed widgets (see [`crate::themed`]) wrap
//! these and fill in appearance from a [`creamui_theme::Theme`]; apps that
//! want a completely custom look can use these directly instead.

use crate::clipboard::write_text as clipboard_write;
use creamui_core::layout::Style;
use creamui_core::{
    BoxedWidget, CursorIcon, Key, KeyInput, Painter, Point, Rect, Styled, TextAlign, Widget,
};
use creamui_theme::Color;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn activate_on_key(click: Rc<dyn Fn()>) -> Rc<dyn Fn(KeyInput)> {
    Rc::new(move |input| {
        if !input.modifiers.ctrl && matches!(input.key, Key::Enter | Key::Char(' ')) {
            click();
        }
    })
}

/// Draws an underline and/or strikethrough rule under/through a run of text
/// painted with [`Painter::fill_text_font`], shared by [`RawText`] and
/// [`RawLink`] so both decorate exactly the same way. `rect`, `font_size`,
/// `bold` and `align` must match the values the text itself was painted
/// with — the rule's width and horizontal position are derived from
/// [`crate::text_metrics::measure_family`] using them, not from re-measuring
/// glyphs the painter already laid out.
fn draw_text_decorations(
    painter: &mut dyn Painter,
    rect: Rect,
    text: &str,
    font_size: f32,
    family: Option<&str>,
    bold: bool,
    align: TextAlign,
    color: Color,
    underline: bool,
    strikethrough: bool,
) {
    if !underline && !strikethrough {
        return;
    }
    let (width, _) = crate::text_metrics::measure_family(text, font_size, rect.width, family, bold);
    let width = width.min(rect.width.max(0.0));
    let thickness = (font_size * 0.06).max(1.0).min(rect.height.max(0.0));
    if width <= 0.0 || thickness <= 0.0 {
        return;
    }
    let x = match align {
        TextAlign::Start => rect.x,
        TextAlign::Center => rect.x + (rect.width - width) / 2.0,
        TextAlign::End => rect.x + rect.width - width,
    };
    let center_y = rect.y + rect.height / 2.0;
    if underline {
        let y = (center_y + font_size * 0.32).clamp(
            rect.y + thickness / 2.0,
            rect.y + rect.height - thickness / 2.0,
        );
        painter.fill_rect(
            Rect {
                x,
                y: y - thickness / 2.0,
                width,
                height: thickness,
            },
            color,
            0.0,
        );
    }
    if strikethrough {
        let y = (center_y - font_size * 0.02).clamp(
            rect.y + thickness / 2.0,
            rect.y + rect.height - thickness / 2.0,
        );
        painter.fill_rect(
            Rect {
                x,
                y: y - thickness / 2.0,
                width,
                height: thickness,
            },
            color,
            0.0,
        );
    }
}

mod actions;
mod application;
mod button;
mod calendar;
mod canvas;
mod carousel;
mod content;
mod context_menu;
mod controls;
mod data_table;
mod dataview;
mod disclosure;
mod foundation;
mod marquee;
mod navigation;
mod number;
mod otp;
mod pickers;
mod questionnaire;
mod rating;
mod resizable;
mod scroll;
mod sheet;
mod spinner;
mod split;
mod text_input;
mod toast;
mod toolbar;
mod tooltip;
mod typography;
mod virtualize;
mod workspace;

pub use actions::*;
pub use application::*;
pub use button::*;
pub use calendar::*;
pub use canvas::*;
pub use carousel::*;
pub use content::*;
pub use context_menu::*;
pub use controls::*;
pub use data_table::*;
pub use dataview::*;
pub use disclosure::*;
pub use foundation::*;
pub use marquee::*;
pub use navigation::*;
pub use number::*;
pub use otp::*;
pub use pickers::*;
pub use questionnaire::*;
pub use rating::*;
pub use resizable::*;
pub use scroll::*;
pub use sheet::*;
pub use spinner::*;
pub use split::*;
pub use text_input::*;
pub use toast::*;
pub use toolbar::*;
pub use tooltip::*;
pub use typography::*;
pub use virtualize::*;
pub use workspace::*;

macro_rules! impl_direct_styled {
    ($($widget:ty),+ $(,)?) => {
        $(
            impl creamui_core::Styled for $widget {
                fn set_style(&mut self, style: creamui_core::Style) {
                    self.style = style;
                }
            }
        )+
    };
}

impl_direct_styled!(
    RawAccordion,
    RawAlert,
    RawAttachment,
    RawButton,
    RawCalendar,
    RawCarousel,
    RawCheckbox,
    RawChatMessage,
    RawCollapsible,
    RawColorPicker,
    RawDateTimePicker,
    RawDescriptionList,
    RawDropdownButton,
    RawEmptyState,
    RawField,
    RawFilePicker,
    RawForm,
    RawInputGroup,
    RawKbd,
    RawLink,
    RawListView,
    RawMarquee,
    RawNumberInput,
    RawOtpInput,
    RawPagination,
    RawPre,
    RawQuestionnaire,
    RawQuote,
    RawRating,
    RawResizablePanels,
    RawScrollView,
    RawScrollbar,
    RawSheet,
    RawSidebar,
    RawSkeleton,
    RawSlider,
    RawSpinner,
    RawStatusBar,
    RawStepper,
    RawSwitch,
    RawTab,
    RawTable,
    RawTag,
    RawTabs,
    RawText,
    RawTextArea,
    RawTextInput,
    RawToast,
    RawToastHost,
    RawTooltip,
    RawView,
    RawVirtualList,
);
