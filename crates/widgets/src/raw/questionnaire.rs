use super::*;
use crate::layout::StyleExt;
use crate::{RawStepper, StepState};
use creamui_reactive::Signal;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct QuestionStep {
    pub id: String,
    pub required: bool,
    next: Option<Rc<dyn Fn(&BTreeMap<String, String>) -> Option<usize>>>,
}

impl QuestionStep {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            required: false,
            next: None,
        }
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn branch(
        mut self,
        next: impl Fn(&BTreeMap<String, String>) -> Option<usize> + 'static,
    ) -> Self {
        self.next = Some(Rc::new(next));
        self
    }
}

#[derive(Clone)]
pub struct QuestionnaireController {
    steps: Rc<Vec<QuestionStep>>,
    current: Signal<usize>,
    history: Signal<Vec<usize>>,
    answers: Signal<BTreeMap<String, String>>,
    error: Signal<Option<String>>,
    complete: Signal<bool>,
}

impl QuestionnaireController {
    pub fn new(steps: Vec<QuestionStep>) -> Self {
        Self {
            steps: Rc::new(steps),
            current: Signal::new(0),
            history: Signal::new(Vec::new()),
            answers: Signal::new(BTreeMap::new()),
            error: Signal::new(None),
            complete: Signal::new(false),
        }
    }

    pub fn current(&self) -> usize {
        self.current.get()
    }

    pub fn answer(&self, id: &str) -> Option<String> {
        self.answers.get().get(id).cloned()
    }

    pub fn set_answer(&self, id: &str, value: impl Into<String>) -> bool {
        if !self.steps.iter().any(|step| step.id == id) {
            return false;
        }
        self.answers.update(|answers| {
            answers.insert(id.to_owned(), value.into());
        });
        self.error.set(None);
        self.complete.set(false);
        true
    }

    pub fn error(&self) -> Option<String> {
        self.error.get()
    }

    pub fn is_complete(&self) -> bool {
        self.complete.get()
    }

    pub fn can_go_back(&self) -> bool {
        !self.history.get().is_empty()
    }

    pub fn previous(&self) -> bool {
        let Some(index) = self.history.peek().last().copied() else {
            return false;
        };
        self.history.update(|history| {
            history.pop();
        });
        self.current.set(index);
        self.error.set(None);
        self.complete.set(false);
        true
    }

    pub fn next(&self) -> bool {
        let index = self.current.peek();
        let Some(step) = self.steps.get(index) else {
            return false;
        };
        let answers = self.answers.peek();
        if step.required
            && answers
                .get(&step.id)
                .is_none_or(|answer| answer.trim().is_empty())
        {
            self.error.set(Some(format!("{} is required", step.id)));
            return false;
        }
        let next = step
            .next
            .as_ref()
            .map_or(Some(index + 1), |branch| branch(&answers));
        match next {
            Some(next) if next < self.steps.len() && next != index => {
                self.history.update(|history| history.push(index));
                self.current.set(next);
                self.error.set(None);
                true
            }
            None => {
                self.complete.set(true);
                self.error.set(None);
                true
            }
            Some(next) if next == self.steps.len() => {
                self.complete.set(true);
                self.error.set(None);
                true
            }
            _ => false,
        }
    }

    pub fn steps(&self) -> &[QuestionStep] {
        &self.steps
    }
}

pub struct RawQuestionnaire {
    pub style: creamui_core::Style,
    controller: QuestionnaireController,
    questions: Vec<Option<BoxedWidget>>,
    previous: Option<BoxedWidget>,
    next: Option<BoxedWidget>,
    error: Option<BoxedWidget>,
    step_color: Color,
}

impl RawQuestionnaire {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        controller: &QuestionnaireController,
    ) -> Self {
        let _ = controller.current();
        let _ = controller.error();
        let _ = controller.is_complete();
        Self {
            style: style.into(),
            controller: controller.clone(),
            questions: (0..controller.steps().len()).map(|_| None).collect(),
            previous: None,
            next: None,
            error: None,
            step_color: Color::rgb(0, 0, 0),
        }
    }

    pub fn question(mut self, index: usize, content: BoxedWidget) -> Self {
        if let Some(slot) = self.questions.get_mut(index) {
            *slot = Some(content);
        }
        self
    }

    pub fn navigation(mut self, previous: BoxedWidget, next: BoxedWidget) -> Self {
        self.previous = Some(previous);
        self.next = Some(next);
        self
    }

    pub fn error(mut self, error: BoxedWidget) -> Self {
        self.error = Some(error);
        self
    }

    pub fn step_color(mut self, color: Color) -> Self {
        self.step_color = color;
        self
    }
}

impl Widget for RawQuestionnaire {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let current = self.controller.current();
        let mut stepper = RawStepper::new(Style::default().flex_row().gap(8.0), |_| {});
        for (index, step) in self.controller.steps().iter().enumerate() {
            stepper = stepper.step(
                Box::new(RawText::new(&step.id, self.step_color, 13.0)),
                if index == current {
                    StepState::Active
                } else {
                    StepState::Disabled
                },
            );
        }
        let mut children: Vec<BoxedWidget> = vec![Box::new(stepper)];
        if let Some(question) = self.questions.get_mut(current).and_then(Option::take) {
            children.push(question);
        }
        if self.controller.error().is_some() {
            children.extend(self.error.take());
        }
        let mut navigation = RawView::new(Style::default().flex_row().gap(12.0));
        if let Some(previous) = self.previous.take() {
            let controller = self.controller.clone();
            navigation = navigation.child(Box::new(
                RawButton::new(Style::default(), move || {
                    controller.previous();
                })
                .disabled(!self.controller.can_go_back())
                .child(previous),
            ));
        }
        if let Some(next) = self.next.take() {
            let controller = self.controller.clone();
            navigation = navigation.child(Box::new(
                RawButton::new(Style::default(), move || {
                    controller.next();
                })
                .disabled(self.controller.is_complete() || self.controller.steps().is_empty())
                .child(next),
            ));
        }
        children.push(Box::new(navigation));
        children
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_answers_and_branches_preserve_history() {
        let controller = QuestionnaireController::new(vec![
            QuestionStep::new("type").required().branch(|answers| {
                (answers.get("type").map(String::as_str) == Some("advanced"))
                    .then_some(1)
                    .or(Some(2))
            }),
            QuestionStep::new("details").required(),
            QuestionStep::new("confirm"),
        ]);
        assert!(!controller.next());
        assert_eq!(controller.error().as_deref(), Some("type is required"));
        assert!(controller.set_answer("type", "basic"));
        assert!(controller.next());
        assert_eq!(controller.current(), 2);
        assert!(controller.previous());
        assert_eq!(controller.current(), 0);
        controller.set_answer("type", "advanced");
        assert!(controller.next());
        assert_eq!(controller.current(), 1);
        assert!(!controller.next());
        controller.set_answer("details", "done");
        assert!(controller.next());
        assert!(controller.next());
        assert!(controller.is_complete());
    }

    #[test]
    fn only_active_question_is_mounted() {
        let controller = QuestionnaireController::new(vec![
            QuestionStep::new("first"),
            QuestionStep::new("second"),
        ]);
        let mut widget = RawQuestionnaire::new(Style::default(), &controller)
            .question(0, Box::new(RawView::new(Style::default())))
            .question(1, Box::new(RawView::new(Style::default())));
        assert_eq!(widget.children().len(), 3);
    }
}
