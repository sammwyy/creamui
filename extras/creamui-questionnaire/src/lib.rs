use creamui_core::layout::Style;
use creamui_core::{BoxedWidget, Painter, Rect, Styled, Widget};
use creamui_theme::{use_theme, Color};
use creamui_widgets::{Form, RawButton, RawText, RawView};
use std::rc::Rc;

pub mod raw {
    use super::*;
    mod questionnaire;
    pub use questionnaire::*;
}

pub mod themed {
    use super::*;
    mod questionnaire;
    pub use questionnaire::*;
}

pub use raw::{QuestionStep, QuestionnaireController, RawQuestionnaire};
pub use themed::Questionnaire;
