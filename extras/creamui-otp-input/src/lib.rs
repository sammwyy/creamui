use creamui_core::layout::{Dimension, Style};
use creamui_core::{CursorIcon, Key, KeyInput, Painter, Point, Rect, Styled, TextAlign, Widget};
use creamui_theme::{use_theme, Color};
use std::cell::Cell;
use std::rc::Rc;

pub mod raw {
    use super::*;
    mod otp;
    pub use otp::*;
}

pub mod themed {
    use super::*;
    mod otp;
    pub use otp::*;
}

pub use raw::{OtpColors, OtpController, RawOtpInput};
pub use themed::OtpInput;
