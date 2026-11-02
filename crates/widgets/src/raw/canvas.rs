use creamui_core::{Painter, Point, Rect, RgbaImage, TextAlign, Widget};
use creamui_theme::Color;
use std::rc::Rc;

pub struct CanvasPaint<'a> {
    painter: &'a mut dyn Painter,
    origin: Point,
    pub width: f32,
    pub height: f32,
}

impl CanvasPaint<'_> {
    fn point(&self, point: Point) -> Point {
        Point {
            x: self.origin.x + point.x,
            y: self.origin.y + point.y,
        }
    }

    fn rect(&self, rect: Rect) -> Rect {
        Rect {
            x: self.origin.x + rect.x,
            y: self.origin.y + rect.y,
            ..rect
        }
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Color, radius: f32) {
        self.painter.fill_rect(self.rect(rect), color, radius);
    }

    pub fn stroke_line(&mut self, from: Point, to: Point, color: Color, width: f32) {
        self.painter
            .stroke_line(self.point(from), self.point(to), color, width);
    }

    pub fn fill_text(
        &mut self,
        rect: Rect,
        text: &str,
        color: Color,
        font_size: f32,
        align: TextAlign,
    ) {
        self.painter
            .fill_text(self.rect(rect), text, color, font_size, align);
    }

    pub fn draw_image(&mut self, rect: Rect, image: &RgbaImage) {
        self.painter.draw_image(self.rect(rect), image, None);
    }

    pub fn scale_factor(&self) -> f32 {
        self.painter.scale_factor()
    }
}

pub struct RawCanvas {
    pub style: creamui_core::Style,
    draw: Rc<dyn Fn(&mut CanvasPaint<'_>)>,
}

impl RawCanvas {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        draw: impl Fn(&mut CanvasPaint<'_>) + 'static,
    ) -> Self {
        Self {
            style: style.into(),
            draw: Rc::new(draw),
        }
    }
}

impl Widget for RawCanvas {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.push_clip(rect);
        (self.draw)(&mut CanvasPaint {
            painter,
            origin: Point {
                x: rect.x,
                y: rect.y,
            },
            width: rect.width,
            height: rect.height,
        });
        painter.pop_clip();
    }
}

impl_styled_field!(RawCanvas);

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::{BoxedWidget, Renderer, Size};

    #[derive(Default)]
    struct TestPainter {
        clips: Vec<Rect>,
        fills: Vec<Rect>,
    }

    impl Painter for TestPainter {
        fn push_clip(&mut self, rect: Rect) {
            self.clips.push(rect);
        }

        fn pop_clip(&mut self) {
            self.clips.pop();
        }

        fn fill_rect(&mut self, rect: Rect, _: Color, _: f32) {
            assert!(self.clips.len() >= 1);
            assert_eq!(self.clips.last().unwrap().width, 40.0);
            self.fills.push(rect);
        }

        fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
        fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
    }

    #[test]
    fn draws_in_local_coordinates_and_restores_clip() {
        let style = creamui_core::layout::Style {
            size: creamui_core::layout::Size {
                width: creamui_core::layout::Dimension::Length(40.0),
                height: creamui_core::layout::Dimension::Length(20.0),
            },
            ..Default::default()
        };
        let child: BoxedWidget = Box::new(RawCanvas::new(style, |canvas| {
            assert_eq!((canvas.width, canvas.height), (40.0, 20.0));
            canvas.fill_rect(
                Rect {
                    x: 2.0,
                    y: 3.0,
                    width: 8.0,
                    height: 5.0,
                },
                Color::rgb(1, 2, 3),
                0.0,
            );
        }));
        let mut renderer = Renderer::new();
        let mut painter = TestPainter::default();
        renderer.render(
            Box::new(
                super::super::RawView::new(creamui_core::layout::Style::default()).child(child),
            ),
            Size {
                width: 100.0,
                height: 100.0,
            },
            &mut painter,
        );
        assert_eq!(painter.fills[0].x, 2.0);
        assert_eq!(painter.fills[0].y, 3.0);
        assert!(painter.clips.is_empty());
    }
}
