use super::*;
use crate::TextController;
use creamui_core::layout::{Dimension, FlexDirection};

fn normalize(value: f64, min: Option<f64>, max: Option<f64>, precision: usize) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }
    let value = value
        .max(min.unwrap_or(f64::NEG_INFINITY))
        .min(max.unwrap_or(f64::INFINITY));
    format!("{:.*}", precision, value).parse().ok()
}

pub struct RawNumberInput {
    pub style: creamui_core::Style,
    controller: TextController,
    value: f64,
    step: f64,
    min: Option<f64>,
    max: Option<f64>,
    precision: usize,
    on_change: Rc<dyn Fn(f64)>,
    text_color: Color,
    button_color: Color,
    font_size: f32,
    button_style: creamui_core::Style,
}

impl RawNumberInput {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        controller: &TextController,
        value: f64,
        on_change: impl Fn(f64) + 'static,
    ) -> Self {
        Self {
            style: style.into(),
            controller: controller.clone(),
            value: if value.is_finite() { value } else { 0.0 },
            step: 1.0,
            min: None,
            max: None,
            precision: 0,
            on_change: Rc::new(on_change),
            text_color: Color::rgb(0, 0, 0),
            button_color: Color::rgb(0, 0, 0),
            font_size: 14.0,
            button_style: Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(28.0),
                    height: Dimension::Length(32.0),
                },
                align_items: Some(creamui_core::layout::AlignItems::Center),
                justify_content: Some(creamui_core::layout::JustifyContent::Center),
                flex_shrink: 0.0,
                ..Default::default()
            }
            .into(),
        }
    }

    pub fn bounds(mut self, min: Option<f64>, max: Option<f64>) -> Self {
        self.min = min.filter(|value| value.is_finite());
        self.max = max.filter(|value| value.is_finite());
        if self.min.zip(self.max).is_some_and(|(min, max)| min > max) {
            self.max = self.min;
        }
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        if step.is_finite() && step > 0.0 {
            self.step = step;
        }
        self
    }

    pub fn precision(mut self, precision: usize) -> Self {
        self.precision = precision.min(10);
        self
    }

    pub fn colors(mut self, text: Color, button: Color, font_size: f32) -> Self {
        self.text_color = text;
        self.button_color = button;
        self.font_size = font_size.max(1.0);
        self
    }

    pub fn button_style(mut self, style: impl Into<creamui_core::Style>) -> Self {
        self.button_style = style.into();
        self
    }

    fn normalized(&self, value: f64) -> Option<f64> {
        normalize(value, self.min, self.max, self.precision)
    }
}

impl Widget for RawNumberInput {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Row;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let value = self.value;
        let minimum = self.min;
        let maximum = self.max;
        let precision = self.precision;
        let controller = self.controller.clone();
        let on_change = self.on_change.clone();
        let commit = Rc::new(move || {
            let next = controller
                .peek()
                .trim()
                .parse::<f64>()
                .ok()
                .and_then(|next| normalize(next, minimum, maximum, precision));
            let next = next.unwrap_or(value);
            controller.set_value(format!("{:.*}", precision, next));
            if next != value {
                on_change(next);
            }
        });
        let draft = self.controller.value();
        let set_draft = self.controller.clone();
        let set_cursor = self.controller.clone();
        let set_selection = self.controller.clone();
        let submit = commit.clone();
        let blur = commit.clone();
        let key_controller = self.controller.clone();
        let key_change = self.on_change.clone();
        let step = self.step;
        let input = RawTextInput::new(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(120.0),
                    height: Dimension::Length(36.0),
                },
                flex_grow: 1.0,
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Auto,
                },
                ..Default::default()
            },
            draft,
            self.font_size,
            self.text_color,
            move |draft| set_draft.set_value(draft),
        )
        .cursor(self.controller.cursor(), move |cursor| {
            set_cursor.set_cursor(cursor)
        })
        .selection(self.controller.selection(), move |selection| {
            set_selection.set_selection(selection)
        })
        .on_submit(move || submit())
        .on_key_press(move |input| {
            let direction = match input.key {
                Key::Up => 1.0,
                Key::Down => -1.0,
                _ => return,
            };
            let current = key_controller
                .peek()
                .parse::<f64>()
                .ok()
                .filter(|current| current.is_finite())
                .unwrap_or(value);
            if let Some(next) = normalize(current + direction * step, minimum, maximum, precision) {
                if next != value {
                    key_controller.set_value(format!("{:.*}", precision, next));
                    key_change(next);
                }
            }
        })
        .on_focus_change(move |focused| {
            if !focused {
                blur();
            }
        });
        let mut children = vec![Box::new(input) as BoxedWidget];
        for direction in [-1.0, 1.0] {
            let next = self
                .normalized(value + direction * self.step)
                .unwrap_or(value);
            let callback = self.on_change.clone();
            let controller = self.controller.clone();
            let label = if direction < 0.0 { "−" } else { "+" };
            children.push(Box::new(
                RawButton::new(self.button_style.clone(), move || {
                    if next != value {
                        controller.set_value(format!("{:.*}", precision, next));
                        callback(next);
                    }
                })
                .disabled(next == value)
                .child(Box::new(RawText::new(
                    label,
                    self.button_color,
                    self.font_size,
                ))),
            ));
        }
        children
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_input_clamps_and_rounds_step_actions() {
        let controller = TextController::new("0.9");
        let reported = Rc::new(Cell::new(0.0));
        let set_reported = reported.clone();
        let mut input = RawNumberInput::new(Style::default(), &controller, 0.9, move |next| {
            set_reported.set(next)
        })
        .bounds(Some(0.0), Some(1.0))
        .step(0.2)
        .precision(1);
        assert_eq!(input.normalized(-1.0), Some(0.0));
        let buttons = input.children();
        buttons[2].on_click().unwrap()();
        assert_eq!(reported.get(), 1.0);
        assert_eq!(controller.peek(), "1.0");
    }

    #[test]
    fn invalid_text_reverts_when_committed() {
        let controller = TextController::new("bad");
        let mut input = RawNumberInput::new(Style::default(), &controller, 5.0, |_| {});
        let parts = input.children();
        parts[0].on_key().unwrap()(KeyInput {
            key: Key::Enter,
            modifiers: Default::default(),
        });
        assert_eq!(controller.peek(), "5");
    }

    #[test]
    fn arrow_keys_step_and_respect_bounds() {
        let controller = TextController::new("4");
        let reported = Rc::new(Cell::new(0.0));
        let target = reported.clone();
        let mut input = RawNumberInput::new(Style::default(), &controller, 4.0, move |next| {
            target.set(next)
        })
        .bounds(Some(0.0), Some(5.0))
        .step(2.0);
        let children = input.children();
        children[0].on_key().unwrap()(KeyInput {
            key: Key::Up,
            modifiers: Default::default(),
        });
        assert_eq!(reported.get(), 5.0);
    }
}
