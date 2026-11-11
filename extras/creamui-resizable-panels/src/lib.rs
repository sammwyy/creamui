use creamui_core::layout::{Dimension, Style};
use creamui_core::{BoxedWidget, Painter, Rect, Styled, Widget};
use creamui_theme::{use_theme, Color};
use creamui_widgets::{SplitAxis, SplitState};

#[cfg(test)]
use creamui_widgets::RawView;

pub mod raw {
    use super::*;
    mod resizable;
    pub use resizable::*;
}

pub mod themed {
    use super::*;
    mod resizable;
    pub use resizable::*;
}

pub use raw::RawResizablePanels;
pub use themed::ResizablePanels;
