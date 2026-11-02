use creamui_core::{Painter, Point, Rect, RgbaImage, TextAlign, WheelInput, Widget};
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

    pub fn translated(&mut self, offset: Point, draw: impl FnOnce(&mut CanvasPaint<'_>)) {
        let origin = self.point(offset);
        draw(&mut CanvasPaint {
            painter: self.painter,
            origin,
            width: self.width,
            height: self.height,
        });
    }

    pub fn clipped(&mut self, rect: Rect, draw: impl FnOnce(&mut CanvasPaint<'_>)) {
        let rect = self.rect(rect);
        self.painter.push_clip(rect);
        draw(self);
        self.painter.pop_clip();
    }
}

pub struct RawCanvas {
    pub style: creamui_core::Style,
    draw: Rc<dyn Fn(&mut CanvasPaint<'_>)>,
    click: Option<Rc<dyn Fn(Point)>>,
    context_menu: Option<Rc<dyn Fn(Point)>>,
    drag_start: Option<Rc<dyn Fn(Point, Rect)>>,
    drag: Option<Rc<dyn Fn(Point, Rect)>>,
    drag_end: Option<Rc<dyn Fn()>>,
    wheel: Option<Rc<dyn Fn(WheelInput)>>,
}

impl RawCanvas {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        draw: impl Fn(&mut CanvasPaint<'_>) + 'static,
    ) -> Self {
        Self {
            style: style.into(),
            draw: Rc::new(draw),
            click: None,
            context_menu: None,
            drag_start: None,
            drag: None,
            drag_end: None,
            wheel: None,
        }
    }

    pub fn on_click(mut self, handler: impl Fn(Point) + 'static) -> Self {
        self.click = Some(Rc::new(handler));
        self
    }

    pub fn on_context_menu(mut self, handler: impl Fn(Point) + 'static) -> Self {
        self.context_menu = Some(Rc::new(handler));
        self
    }

    pub fn on_drag_start(mut self, handler: impl Fn(Point, Rect) + 'static) -> Self {
        self.drag_start = Some(Rc::new(handler));
        self
    }

    pub fn on_drag(mut self, handler: impl Fn(Point, Rect) + 'static) -> Self {
        self.drag = Some(Rc::new(handler));
        self
    }

    pub fn on_drag_end(mut self, handler: impl Fn() + 'static) -> Self {
        self.drag_end = Some(Rc::new(handler));
        self
    }

    pub fn on_wheel(mut self, handler: impl Fn(WheelInput) + 'static) -> Self {
        self.wheel = Some(Rc::new(handler));
        self
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

    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        self.click.clone()
    }

    fn on_context_menu(&self) -> Option<Rc<dyn Fn(Point)>> {
        self.context_menu.clone()
    }

    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.drag_start.clone()
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.drag.clone()
    }

    fn on_drag_end(&self) -> Option<Rc<dyn Fn()>> {
        self.drag_end.clone()
    }

    fn on_wheel(&self) -> Option<Rc<dyn Fn(WheelInput)>> {
        self.wheel.clone()
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
        clip_history: Vec<Rect>,
        fills: Vec<Rect>,
    }

    impl Painter for TestPainter {
        fn push_clip(&mut self, rect: Rect) {
            self.clips.push(rect);
            self.clip_history.push(rect);
        }

        fn pop_clip(&mut self) {
            self.clips.pop();
        }

        fn fill_rect(&mut self, rect: Rect, _: Color, _: f32) {
            assert!(self.clips.len() >= 1);
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

    #[test]
    fn translated_clipped_drawing_and_pointer_handlers_share_local_coordinates() {
        let drag = Rc::new(std::cell::Cell::new(None));
        let widget = RawCanvas::new(creamui_core::layout::Style::default(), |canvas| {
            canvas.translated(Point { x: 5.0, y: 6.0 }, |canvas| {
                canvas.clipped(
                    Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    |canvas| {
                        canvas.fill_rect(
                            Rect {
                                x: 1.0,
                                y: 2.0,
                                width: 3.0,
                                height: 4.0,
                            },
                            Color::rgb(0, 0, 0),
                            0.0,
                        );
                    },
                );
            });
        })
        .on_drag({
            let drag = drag.clone();
            move |point, _| drag.set(Some(point))
        });
        let rect = Rect {
            x: 20.0,
            y: 30.0,
            width: 40.0,
            height: 20.0,
        };
        let mut painter = TestPainter::default();
        widget.paint(&mut painter, rect);
        assert_eq!(painter.fills[0].x, 26.0);
        assert_eq!(painter.fills[0].y, 38.0);
        assert_eq!(painter.clip_history[1].x, 25.0);
        assert_eq!(painter.clip_history[1].y, 36.0);
        assert!(painter.clips.is_empty());
        Widget::on_drag(&widget).unwrap()(Point { x: -2.0, y: 22.0 }, rect);
        assert_eq!(drag.get(), Some(Point { x: -2.0, y: 22.0 }));
    }
}
