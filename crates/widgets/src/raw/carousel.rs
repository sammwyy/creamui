use super::*;
use creamui_core::layout::{Dimension, FlexDirection, JustifyContent};

pub struct RawCarousel {
    pub style: creamui_core::Style,
    index: usize,
    slides: Vec<BoxedWidget>,
    on_change: Rc<dyn Fn(usize)>,
    looping: bool,
    controls: Option<(BoxedWidget, BoxedWidget)>,
    indicator_color: Color,
    selected_color: Color,
    drag_origin: Rc<Cell<Option<f32>>>,
    drag_destination: Rc<Cell<Option<f32>>>,
}

impl RawCarousel {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        index: usize,
        on_change: impl Fn(usize) + 'static,
    ) -> Self {
        Self {
            style: style.into(),
            index,
            slides: Vec::new(),
            on_change: Rc::new(on_change),
            looping: false,
            controls: None,
            indicator_color: Color::rgb(160, 160, 160),
            selected_color: Color::rgb(35, 90, 180),
            drag_origin: Rc::new(Cell::new(None)),
            drag_destination: Rc::new(Cell::new(None)),
        }
    }

    pub fn slide(mut self, slide: BoxedWidget) -> Self {
        self.slides.push(slide);
        self
    }

    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    pub fn controls(mut self, previous: BoxedWidget, next: BoxedWidget) -> Self {
        self.controls = Some((previous, next));
        self
    }

    pub fn indicators(mut self, inactive: Color, active: Color) -> Self {
        self.indicator_color = inactive;
        self.selected_color = active;
        self
    }

    fn move_to(&self, offset: isize) -> usize {
        let count = self.slides.len();
        if count == 0 {
            return 0;
        }
        let index = self.index.min(count - 1);
        if self.looping {
            (index as isize + offset).rem_euclid(count as isize) as usize
        } else {
            index.saturating_add_signed(offset).min(count - 1)
        }
    }
}

impl Widget for RawCarousel {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Column;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn focusable(&self) -> bool {
        self.slides.len() > 1
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        if self.slides.len() < 2 {
            return None;
        }
        let previous = self.move_to(-1);
        let next = self.move_to(1);
        let callback = self.on_change.clone();
        Some(Rc::new(move |input| match input.key {
            Key::Left => callback(previous),
            Key::Right => callback(next),
            _ => {}
        }))
    }

    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        if self.slides.len() < 2 {
            return None;
        }
        let origin = self.drag_origin.clone();
        let destination = self.drag_destination.clone();
        Some(Rc::new(move |point, _| {
            origin.set(Some(point.x));
            destination.set(Some(point.x));
        }))
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        if self.slides.len() < 2 {
            return None;
        }
        let destination = self.drag_destination.clone();
        Some(Rc::new(move |point, _| destination.set(Some(point.x))))
    }

    fn on_drag_end(&self) -> Option<Rc<dyn Fn()>> {
        if self.slides.len() < 2 {
            return None;
        }
        let origin = self.drag_origin.clone();
        let destination = self.drag_destination.clone();
        let previous = self.move_to(-1);
        let next = self.move_to(1);
        let callback = self.on_change.clone();
        Some(Rc::new(move || {
            if let (Some(start), Some(end)) = (origin.replace(None), destination.replace(None)) {
                if (end - start).abs() > 36.0 {
                    callback(if end < start { next } else { previous });
                }
            }
        }))
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        if self.slides.is_empty() {
            return Vec::new();
        }
        let index = self.index.min(self.slides.len() - 1);
        let count = self.slides.len();
        let index_before = self.move_to(-1);
        let index_after = self.move_to(1);
        let mut children = vec![self.slides.swap_remove(index)];
        let mut navigation = RawView::new(Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            flex_direction: FlexDirection::Row,
            justify_content: Some(JustifyContent::SpaceBetween),
            ..Default::default()
        });
        let (previous, next) = match self.controls.take() {
            Some((previous, next)) => (Some(previous), Some(next)),
            None => (None, None),
        };
        if let Some(previous) = previous {
            let on_previous = self.on_change.clone();
            navigation = navigation.child(Box::new(
                RawButton::new(Style::default(), move || on_previous(index_before))
                    .disabled(index_before == index)
                    .child(previous),
            ));
        }
        let mut indicators = RawView::new(Style {
            flex_direction: FlexDirection::Row,
            ..Default::default()
        });
        for current in 0..count {
            let callback = self.on_change.clone();
            indicators = indicators.child(Box::new(
                RawButton::new(
                    Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Length(20.0),
                            height: Dimension::Length(20.0),
                        },
                        ..Default::default()
                    },
                    move || callback(current),
                )
                .child(Box::new(
                    RawView::new(Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Length(8.0),
                            height: Dimension::Length(8.0),
                        },
                        ..Default::default()
                    })
                    .background(if current == index {
                        self.selected_color
                    } else {
                        self.indicator_color
                    })
                    .corner_radius(4.0),
                )),
            ));
        }
        navigation = navigation.child(Box::new(indicators));
        if let Some(next) = next {
            let on_next = self.on_change.clone();
            navigation = navigation.child(Box::new(
                RawButton::new(Style::default(), move || on_next(index_after))
                    .disabled(index_after == index)
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

    fn slide() -> BoxedWidget {
        Box::new(RawView::new(Style::default()))
    }

    #[test]
    fn carousel_clamps_or_wraps_and_mounts_only_selected_slide() {
        let mut finite = RawCarousel::new(Style::default(), 2, |_| {})
            .slide(slide())
            .slide(slide())
            .slide(slide());
        assert_eq!(finite.move_to(1), 2);
        let mut parts = finite.children();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1].children().len(), 1);
        let looped = RawCarousel::new(Style::default(), 0, |_| {})
            .slide(slide())
            .slide(slide())
            .looping(true);
        assert_eq!(looped.move_to(-1), 1);
    }

    #[test]
    fn swipe_and_keyboard_select_next() {
        let selected = Rc::new(Cell::new(0usize));
        let target = selected.clone();
        let carousel = RawCarousel::new(Style::default(), 0, move |index| target.set(index))
            .slide(slide())
            .slide(slide());
        carousel.on_drag_start().unwrap()(Point { x: 100.0, y: 0.0 }, Rect::default());
        carousel.on_drag().unwrap()(Point { x: 20.0, y: 0.0 }, Rect::default());
        carousel.on_drag_end().unwrap()();
        assert_eq!(selected.get(), 1);
        carousel.on_key().unwrap()(KeyInput {
            key: Key::Right,
            modifiers: Default::default(),
        });
        assert_eq!(selected.get(), 1);
    }
}
