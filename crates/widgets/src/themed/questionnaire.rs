use super::*;
use crate::layout::StyleExt;
use crate::raw::{QuestionnaireController, RawQuestionnaire};

pub struct Questionnaire {
    inner: RawQuestionnaire,
}
impl_styled_inner!(Questionnaire);

impl Questionnaire {
    pub fn new(controller: &QuestionnaireController) -> Self {
        let theme = use_theme();
        let previous = Box::new(RawText::new(
            "Previous",
            theme.text_secondary,
            theme.typography.body,
        ));
        let next = Box::new(RawText::new(
            if controller.current() + 1 == controller.steps().len() {
                "Finish"
            } else {
                "Next"
            },
            theme.accent,
            theme.typography.body,
        ));
        let mut inner = RawQuestionnaire::new(
            Style::default().flex_column().gap(theme.spacing_medium),
            controller,
        )
        .step_color(theme.text_primary)
        .navigation(previous, next);
        if let Some(error) = controller.error() {
            inner = inner.error(Box::new(RawText::new(
                error,
                theme.danger,
                theme.typography.caption,
            )));
        }
        Self { inner }
    }

    pub fn question(mut self, index: usize, content: BoxedWidget) -> Self {
        self.inner = self
            .inner
            .question(index, Box::new(Form::new().field(content)));
        self
    }
}

impl Widget for Questionnaire {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::QuestionStep;

    #[test]
    fn themed_questionnaire_mounts_active_step() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let controller = QuestionnaireController::new(vec![QuestionStep::new("Choice")]);
            let mut questionnaire = Questionnaire::new(&controller)
                .question(0, Box::new(RawView::new(Style::default())));
            assert_eq!(questionnaire.children().len(), 3);
        });
    }
}
