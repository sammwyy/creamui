use super::*;
use crate::raw::{OtpColors, OtpController, RawOtpInput};

pub struct OtpInput {
    inner: RawOtpInput,
}
impl Styled for OtpInput {
    fn set_style(&mut self, style: creamui_core::Style) {
        self.inner.set_style(self.inner.style().merged_over(style));
    }
}

impl OtpInput {
    pub fn new(controller: &OtpController, on_change: impl Fn(String) + 'static) -> Self {
        let theme = use_theme();
        Self {
            inner: RawOtpInput::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(42.0),
                    },
                    max_size: creamui_core::layout::Size {
                        width: Dimension::Length(controller.length() as f32 * 40.0),
                        height: Dimension::Auto,
                    },
                    min_size: creamui_core::layout::Size {
                        width: Dimension::Length(0.0),
                        height: Dimension::Auto,
                    },
                    ..Default::default()
                },
                controller,
                on_change,
            )
            .colors(OtpColors {
                text: theme.text_primary,
                surface: theme.surface_elevated,
                border: theme.border,
                focus: theme.accent,
                error: theme.danger,
            }),
        }
    }

    pub fn error(mut self, error: bool) -> Self {
        self.inner = self.inner.error(error);
        self
    }

    pub fn masked(mut self, masked: bool) -> Self {
        self.inner = self.inner.masked(masked);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.inner = self.inner.disabled(disabled);
        self
    }
}

impl Widget for OtpInput {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn style_state(&self) -> creamui_core::StyleState {
        self.inner.style_state()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn focusable(&self) -> bool {
        self.inner.focusable()
    }

    fn accepts_text_input(&self) -> bool {
        self.inner.accepts_text_input()
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.inner.on_layout(rect)
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }

    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        self.inner.on_click_at()
    }

    fn on_focus_change(&self) -> Option<Rc<dyn Fn(bool)>> {
        self.inner.on_focus_change()
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themed_otp_renders_cells_and_keeps_input_focusable() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let controller = OtpController::new(6, "123");
            let widget = OtpInput::new(&controller, |_| {});
            assert!(widget.focusable());
            assert!(widget.accepts_text_input());
            assert_eq!(
                widget.style().layout.max_size.width,
                Dimension::Length(240.0)
            );
        });
    }
}
