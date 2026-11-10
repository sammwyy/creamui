use super::*;
use creamui_core::layout::{Dimension, FlexDirection};

pub struct RawDescriptionList {
    pub style: creamui_core::Style,
    entries: Vec<(BoxedWidget, BoxedWidget)>,
    term_width: Option<f32>,
}

impl RawDescriptionList {
    pub fn new(style: impl Into<creamui_core::Style>) -> Self {
        Self {
            style: style.into(),
            entries: Vec::new(),
            term_width: None,
        }
    }

    pub fn entry(mut self, term: BoxedWidget, value: BoxedWidget) -> Self {
        self.entries.push((term, value));
        self
    }

    pub fn term_width(mut self, width: f32) -> Self {
        self.term_width = Some(width.max(0.0));
        self
    }
}

impl Widget for RawDescriptionList {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Column;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.entries)
            .into_iter()
            .map(|(term, value)| {
                let mut term_style = Style::default();
                if let Some(width) = self.term_width {
                    term_style.size.width = Dimension::Length(width);
                    term_style.flex_shrink = 0.0;
                }
                Box::new(
                    RawView::new(Style {
                        flex_direction: if self.term_width.is_some() {
                            FlexDirection::Row
                        } else {
                            FlexDirection::Column
                        },
                        ..Default::default()
                    })
                    .child(Box::new(RawView::new(term_style).child(term)))
                    .child(Box::new(
                        RawView::new(Style {
                            flex_grow: 1.0,
                            min_size: creamui_core::layout::Size {
                                width: Dimension::Length(0.0),
                                height: Dimension::Auto,
                            },
                            ..Default::default()
                        })
                        .child(value),
                    )),
                ) as BoxedWidget
            })
            .collect()
    }
}

pub struct RawStatusBar {
    pub style: creamui_core::Style,
    start: Option<BoxedWidget>,
    center: Option<BoxedWidget>,
    end: Option<BoxedWidget>,
}

impl RawStatusBar {
    pub fn new(style: impl Into<creamui_core::Style>) -> Self {
        Self {
            style: style.into(),
            start: None,
            center: None,
            end: None,
        }
    }

    pub fn start(mut self, widget: BoxedWidget) -> Self {
        self.start = Some(widget);
        self
    }

    pub fn center(mut self, widget: BoxedWidget) -> Self {
        self.center = Some(widget);
        self
    }

    pub fn end(mut self, widget: BoxedWidget) -> Self {
        self.end = Some(widget);
        self
    }
}

impl Widget for RawStatusBar {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Row;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        [self.start.take(), self.center.take(), self.end.take()]
            .into_iter()
            .enumerate()
            .map(|(index, slot)| {
                Box::new(
                    RawView::new(Style {
                        flex_grow: 1.0,
                        flex_basis: Dimension::Length(0.0),
                        min_size: creamui_core::layout::Size {
                            width: Dimension::Length(0.0),
                            height: Dimension::Auto,
                        },
                        justify_content: Some(match index {
                            0 => creamui_core::layout::JustifyContent::Start,
                            1 => creamui_core::layout::JustifyContent::Center,
                            _ => creamui_core::layout::JustifyContent::End,
                        }),
                        ..Default::default()
                    })
                    .with_children(slot.into_iter().collect()),
                ) as BoxedWidget
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Upcoming,
    Active,
    Completed,
    Disabled,
}

pub struct RawStepper {
    pub style: creamui_core::Style,
    steps: Vec<(BoxedWidget, StepState)>,
    on_select: Rc<dyn Fn(usize)>,
}

impl RawStepper {
    pub fn new(style: impl Into<creamui_core::Style>, on_select: impl Fn(usize) + 'static) -> Self {
        Self {
            style: style.into(),
            steps: Vec::new(),
            on_select: Rc::new(on_select),
        }
    }

    pub fn step(mut self, label: BoxedWidget, state: StepState) -> Self {
        self.steps.push((label, state));
        self
    }
}

impl Widget for RawStepper {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.steps)
            .into_iter()
            .enumerate()
            .map(|(index, (label, state))| {
                let callback = self.on_select.clone();
                Box::new(
                    RawButton::new(Style::default(), move || callback(index))
                        .disabled(state == StepState::Disabled)
                        .child(label),
                ) as BoxedWidget
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot() -> BoxedWidget {
        Box::new(RawView::new(Style::default()))
    }

    #[test]
    fn description_list_aligns_terms_without_dropping_entries() {
        let mut list = RawDescriptionList::new(Style::default())
            .term_width(80.0)
            .entry(slot(), slot())
            .entry(slot(), slot());
        let mut rows = list.children();
        assert_eq!(rows.len(), 2);
        let columns = rows[0].children();
        assert_eq!(
            columns[0].style().layout.size.width,
            Dimension::Length(80.0)
        );
    }

    #[test]
    fn status_bar_keeps_three_slots_even_when_empty() {
        let mut bar = RawStatusBar::new(Style::default()).center(slot());
        let mut slots = bar.children();
        assert_eq!(slots.len(), 3);
        assert!(slots[0].children().is_empty());
        assert_eq!(slots[1].children().len(), 1);
    }

    #[test]
    fn disabled_steps_cannot_activate() {
        let selected = Rc::new(Cell::new(None));
        let set_selected = selected.clone();
        let mut stepper =
            RawStepper::new(Style::default(), move |index| set_selected.set(Some(index)))
                .step(slot(), StepState::Active)
                .step(slot(), StepState::Disabled);
        let children = stepper.children();
        children[0].on_click().unwrap()();
        assert_eq!(selected.get(), Some(0));
        assert!(children[1].on_click().is_none());
    }
}
