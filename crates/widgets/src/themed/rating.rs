use super::*;
use crate::raw::RawRating;

pub struct Rating {
    inner: RawRating,
}
impl_styled_inner!(Rating);

impl Rating {
    pub fn new(value: f32, max: usize, on_change: impl Fn(f32) + 'static) -> Self {
        let theme = use_theme();
        Self {
            inner: RawRating::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(max.clamp(1, 20) as f32 * 32.0),
                        height: Dimension::Length(32.0),
                    },
                    ..Default::default()
                },
                value,
                max,
                on_change,
            )
            .colors(theme.accent, theme.border)
            .focus_style(creamui_core::StateStyle::new().outline(theme.accent, 2.0)),
        }
    }

    pub fn half_steps(mut self) -> Self {
        self.inner = self.inner.step(0.5);
        self
    }

    pub fn read_only(mut self) -> Self {
        self.inner = self.inner.read_only(true);
        self
    }
}

impl Widget for Rating {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn style_state(&self) -> creamui_core::StyleState {
        self.inner.style_state()
    }
    fn focusable(&self) -> bool {
        self.inner.focusable()
    }
    fn on_layout(&self, rect: Rect) -> bool {
        self.inner.on_layout(rect)
    }
    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        self.inner.on_click_at()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themed_rating_forwards_interaction_and_read_only_state() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            assert!(Rating::new(3.0, 5, |_| {}).half_steps().on_key().is_some());
            assert!(Rating::new(3.0, 5, |_| {}).read_only().on_key().is_none());
        });
    }
}
