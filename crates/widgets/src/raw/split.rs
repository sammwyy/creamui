use super::*;
use creamui_core::layout::{Dimension, Display, FlexDirection, Size as LayoutSize, Style};
use creamui_reactive::Signal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone)]
pub struct SplitState {
    fraction: Signal<f32>,
    bounds: Rc<Cell<Rect>>,
}

impl SplitState {
    pub fn new(fraction: f32) -> Self {
        Self {
            fraction: Signal::new(fraction.clamp(0.0, 1.0)),
            bounds: Rc::new(Cell::new(Rect::default())),
        }
    }

    pub fn fraction(&self) -> f32 {
        self.fraction.get()
    }

    pub fn set_fraction(&self, fraction: f32) {
        if fraction.is_finite() {
            self.fraction.set_if_changed(fraction.clamp(0.0, 1.0));
        }
    }
}

pub struct RawSplitView {
    pub style: creamui_core::Style,
    state: SplitState,
    axis: SplitAxis,
    first: Option<BoxedWidget>,
    second: Option<BoxedWidget>,
    divider: f32,
    first_min: f32,
    second_min: f32,
    color: Color,
}

impl RawSplitView {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        state: SplitState,
        axis: SplitAxis,
        first: BoxedWidget,
        second: BoxedWidget,
    ) -> Self {
        Self {
            style: style.into(),
            state,
            axis,
            first: Some(first),
            second: Some(second),
            divider: 6.0,
            first_min: 0.0,
            second_min: 0.0,
            color: Color::rgb(90, 90, 90),
        }
    }

    pub fn divider(mut self, width: f32, color: Color) -> Self {
        self.divider = width.max(1.0);
        self.color = color;
        self
    }

    pub fn min_sizes(mut self, first: f32, second: f32) -> Self {
        self.first_min = first.max(0.0);
        self.second_min = second.max(0.0);
        self
    }
}

impl Widget for RawSplitView {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.display = Display::Flex;
        style.layout.flex_direction = match self.axis {
            SplitAxis::Horizontal => FlexDirection::Row,
            SplitAxis::Vertical => FlexDirection::Column,
        };
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn reports_layout(&self) -> bool {
        true
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.state.bounds.set(rect);
        false
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        let fraction = self.state.fraction();
        let pane = |weight: f32, min: f32, child: BoxedWidget| {
            let mut style = Style {
                flex_grow: weight,
                flex_shrink: 1.0,
                flex_basis: Dimension::Length(0.0),
                ..Default::default()
            };
            style.min_size = match self.axis {
                SplitAxis::Horizontal => LayoutSize {
                    width: Dimension::Length(min),
                    height: Dimension::Length(0.0),
                },
                SplitAxis::Vertical => LayoutSize {
                    width: Dimension::Length(0.0),
                    height: Dimension::Length(min),
                },
            };
            Box::new(super::RawView::new(style).child(child)) as BoxedWidget
        };
        let divider_style = Style {
            flex_shrink: 0.0,
            size: match self.axis {
                SplitAxis::Horizontal => LayoutSize {
                    width: Dimension::Length(self.divider),
                    height: Dimension::Percent(1.0),
                },
                SplitAxis::Vertical => LayoutSize {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(self.divider),
                },
            },
            ..Default::default()
        };
        vec![
            pane(fraction, self.first_min, self.first.take().unwrap()),
            Box::new(SplitDivider {
                style: divider_style.into(),
                state: self.state.clone(),
                axis: self.axis,
                first_min: self.first_min,
                second_min: self.second_min,
                color: self.color,
            }),
            pane(1.0 - fraction, self.second_min, self.second.take().unwrap()),
        ]
    }
}

struct SplitDivider {
    style: creamui_core::Style,
    state: SplitState,
    axis: SplitAxis,
    first_min: f32,
    second_min: f32,
    color: Color,
}

impl SplitDivider {
    fn move_to(&self, position: f32) {
        let bounds = self.state.bounds.get();
        let (origin, length, divider) = match self.axis {
            SplitAxis::Horizontal => (bounds.x, bounds.width, self.style.layout.size.width),
            SplitAxis::Vertical => (bounds.y, bounds.height, self.style.layout.size.height),
        };
        let Dimension::Length(divider) = divider else {
            return;
        };
        let available = (length - divider).max(0.0);
        if available == 0.0 {
            return;
        }
        let min = self.first_min.min(available);
        let max = (available - self.second_min).max(min);
        self.state
            .set_fraction(((position - origin - divider / 2.0).clamp(min, max)) / available);
    }
}

impl Widget for SplitDivider {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.fill_rect(rect, self.color, 0.0);
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        let state = self.state.clone();
        let axis = self.axis;
        let first_min = self.first_min;
        let second_min = self.second_min;
        let color = self.color;
        let style = self.style.clone();
        Some(Rc::new(move |point, rect| {
            let divider = SplitDivider {
                style: style.clone(),
                state: state.clone(),
                axis,
                first_min,
                second_min,
                color,
            };
            divider.move_to(match axis {
                SplitAxis::Horizontal => rect.x + point.x,
                SplitAxis::Vertical => rect.y + point.y,
            });
        }))
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(match self.axis {
            SplitAxis::Horizontal => CursorIcon::ResizeHorizontal,
            SplitAxis::Vertical => CursorIcon::ResizeVertical,
        })
    }
}

impl_styled_field!(RawSplitView);

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::{Renderer, Size, TextAlign};

    struct Pane {
        size: Rc<Cell<Rect>>,
    }

    impl Widget for Pane {
        fn style(&self) -> creamui_core::Style {
            Style {
                size: LayoutSize {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Percent(1.0),
                },
                ..Default::default()
            }
            .into()
        }

        fn paint(&self, _: &mut dyn Painter, _: Rect) {}

        fn reports_layout(&self) -> bool {
            true
        }

        fn on_layout(&self, rect: Rect) -> bool {
            self.size.set(rect);
            false
        }
    }

    struct BlankPainter;

    impl Painter for BlankPainter {
        fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
        fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
        fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
    }

    #[test]
    fn panes_follow_persisted_split_fraction() {
        let first = Rc::new(Cell::new(Rect::default()));
        let second = Rc::new(Cell::new(Rect::default()));
        let state = SplitState::new(0.25);
        let mut renderer = Renderer::new();
        let size = Size {
            width: 200.0,
            height: 100.0,
        };
        let build = || {
            Box::new(
                RawSplitView::new(
                    Style {
                        size: LayoutSize {
                            width: Dimension::Length(200.0),
                            height: Dimension::Length(100.0),
                        },
                        ..Default::default()
                    },
                    state.clone(),
                    SplitAxis::Horizontal,
                    Box::new(Pane {
                        size: first.clone(),
                    }),
                    Box::new(Pane {
                        size: second.clone(),
                    }),
                )
                .divider(8.0, Color::rgb(0, 0, 0)),
            ) as BoxedWidget
        };
        renderer.render(build(), size, &mut BlankPainter);
        assert_eq!(first.get().width, 48.0);
        assert_eq!(second.get().width, 144.0);
        state.set_fraction(0.75);
        renderer.render(build(), size, &mut BlankPainter);
        assert_eq!(first.get().width, 144.0);
        assert_eq!(second.get().x, 152.0);
    }

    #[test]
    fn dragging_respects_minimum_pane_sizes() {
        let state = SplitState::new(0.5);
        state.bounds.set(Rect {
            x: 10.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
        });
        let divider = SplitDivider {
            style: Style {
                size: LayoutSize {
                    width: Dimension::Length(10.0),
                    height: Dimension::Auto,
                },
                ..Default::default()
            }
            .into(),
            state: state.clone(),
            axis: SplitAxis::Horizontal,
            first_min: 50.0,
            second_min: 60.0,
            color: Color::rgb(0, 0, 0),
        };
        divider.move_to(10.0);
        assert_eq!(state.fraction(), 50.0 / 190.0);
        divider.move_to(210.0);
        assert_eq!(state.fraction(), 130.0 / 190.0);
        state.set_fraction(f32::NAN);
        assert_eq!(state.fraction(), 130.0 / 190.0);
    }
}
