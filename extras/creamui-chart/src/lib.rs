use creamui_core::layout::{Dimension, Style};
use creamui_core::{BoxedWidget, Painter, Point, Rect, Styled, TextAlign, Widget};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::{RawText, RawView};
use std::cell::Cell;
use std::rc::Rc;

pub mod raw {
    use super::*;
    mod plot;
    pub use plot::*;
}

pub mod themed {
    use super::*;
    mod chart;
    pub use chart::*;
}

pub use raw::{PlotController, PlotHit, PlotKind, PlotScale, PlotSeries, RawPlot};
pub use themed::Chart;
