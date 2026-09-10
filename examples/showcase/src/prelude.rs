//! Single import for every panel/helper module: the widgets, layout, and
//! reactive types the showcase uses, plus the shared helpers in `common`
//! and `nav`. Each file under `panels/` starts with `use crate::prelude::*;`
//! instead of hand-picking imports.

pub use creamui_core::layout::{AlignItems, Dimension, FlexDirection, JustifyContent, Style};
pub use creamui_core::{BoxedWidget, Size, TextAlign};
pub use creamui_image::{ImageData, ImageFit};
pub use creamui_macros::{component, jsx};
pub use creamui_reactive::{create_effect, Effect, Signal};
pub use creamui_render::{run, WindowHandle, WindowOptions};
pub use creamui_theme::{use_theme, Color, SelectionStyle, Theme};
pub use creamui_widgets::layout::{
    column, fixed, padding, row, Align, Justify, StyleExt, Track, Wrap,
};
pub use creamui_widgets::{
    tab_styles, Button, ButtonSize, ButtonState, ButtonVariant, ColorPickerController, DateTime,
    DateTimeController, RawText, RawView, ScrollController, ScrollView, SelectController,
    SurfaceRole, Symbol, TabColors, TabController, TabSizing, TableColumn, Text, TextController,
    TextInput, TextSize, TreeController, TreeNode,
};
pub use std::cell::RefCell;
pub use std::rc::Rc;

pub use crate::common::*;
pub use crate::kit::*;
pub use crate::nav::*;
