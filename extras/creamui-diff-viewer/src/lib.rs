use creamui_core::layout::{Dimension, Style};
use creamui_core::{BoxedWidget, Painter, Point, Rect, Styled, TextAlign, Widget};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::{Button, ButtonSize, RawView};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub mod raw {
    use super::*;
    mod diff;
    pub use diff::*;
}

pub mod themed {
    use super::*;
    mod diff;
    pub use diff::*;
}

pub use raw::{DiffColors, DiffController, DiffHunk, DiffKind, DiffLine, DiffView, RawDiff};
pub use themed::Diff;
