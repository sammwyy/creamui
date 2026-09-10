use super::*;
/// A compact segmented circular progress indicator. Each rebuild may choose a
/// different phase to animate it; it remains useful as a static busy glyph.
pub struct RawSpinner {
    pub style: creamui_core::Style,
    pub phase: usize,
    pub animate: bool,
}
impl RawSpinner {
    pub fn new(color: Color) -> Self {
        Self {
            style: creamui_core::Style::from(Style::default())
                .color(color)
                .width(14.0)
                .height(14.0),
            phase: 0,
            animate: true,
        }
    }
    pub fn phase(mut self, phase: usize) -> Self {
        self.phase = phase;
        self.animate = false;
        self
    }
    pub fn size(mut self, size: f32) -> Self {
        self.style = self.style.width(size).height(size);
        self
    }
}
impl Widget for RawSpinner {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let phase = if self.animate {
            (painter.animation_time() * 10.) as usize
        } else {
            self.phase
        };
        let cx = rect.x + rect.width / 2.0;
        let cy = rect.y + rect.height / 2.0;
        let radius = rect.width.min(rect.height) * 0.36;
        let dot = (rect.width.min(rect.height) * 0.18).max(1.5);
        let colors = painter.color_scheme();
        let color = self
            .style
            .typography
            .color
            .map(|value| value.resolve(&colors))
            .unwrap_or(Color::rgb(0, 0, 0));
        for index in 0..8 {
            let angle = (index as f32 / 8.0) * std::f32::consts::TAU;
            let alpha = if index == phase % 8 { 255 } else { 80 };
            painter.fill_rect(
                Rect {
                    x: cx + angle.cos() * radius - dot / 2.,
                    y: cy + angle.sin() * radius - dot / 2.,
                    width: dot,
                    height: dot,
                },
                Color::rgba(color.r, color.g, color.b, alpha),
                dot / 2.,
            );
        }
    }
}
