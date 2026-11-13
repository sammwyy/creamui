use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::RawNumberInput;
use crate::TextController;

pub struct NumberInput {
    inner: RawNumberInput,
}
impl_styled_inner!(NumberInput);

impl NumberInput {
    pub fn new(controller: &TextController, value: f64, on_change: impl Fn(f64) + 'static) -> Self {
        let theme = use_theme();
        Self {
            inner: RawNumberInput::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(186.0),
                        height: Dimension::Length(38.0),
                    },
                    ..Style::default()
                        .flex_row()
                        .align(Align::Center)
                        .padding_xy(theme.spacing_small, 0.0)
                },
                controller,
                value,
                on_change,
            )
            .colors(
                theme.text_primary,
                theme.text_secondary,
                theme.typography.body,
            )
            .background(theme.surface_elevated)
            .border(theme.border, theme.input_border_width)
            .corner_radius(theme.input_radius),
        }
    }

    pub fn bounds(mut self, min: Option<f64>, max: Option<f64>) -> Self {
        self.inner = self.inner.bounds(min, max);
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        self.inner = self.inner.step(step);
        self
    }

    pub fn precision(mut self, precision: usize) -> Self {
        self.inner = self.inner.precision(precision);
        self
    }
}

impl Widget for NumberInput {
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

    #[test]
    fn themed_number_input_keeps_bounds_and_stepper_children() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let controller = TextController::new("2");
            let mut number = NumberInput::new(&controller, 2.0, |_| {})
                .bounds(Some(0.0), Some(10.0))
                .step(2.0);
            assert_eq!(number.children().len(), 3);
        });
    }
}
