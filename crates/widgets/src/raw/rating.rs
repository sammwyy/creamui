use super::*;

fn hit_value(x: f32, rect: Rect, max: usize, step: f32) -> f32 {
    let width = rect.width / max as f32;
    if width <= 0.0 {
        return 0.0;
    }
    let fraction = ((x - rect.x) / width).clamp(0.0, max as f32);
    ((fraction / step).ceil() * step).clamp(step, max as f32)
}

pub struct RawRating {
    pub style: creamui_core::Style,
    value: f32,
    max: usize,
    step: f32,
    read_only: bool,
    fill: Color,
    empty: Color,
    on_change: Rc<dyn Fn(f32)>,
    bounds: Rc<Cell<Rect>>,
}

impl RawRating {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        value: f32,
        max: usize,
        on_change: impl Fn(f32) + 'static,
    ) -> Self {
        Self {
            style: style.into(),
            value: if value.is_finite() { value } else { 0.0 },
            max: max.clamp(1, 20),
            step: 1.0,
            read_only: false,
            fill: Color::rgb(255, 170, 0),
            empty: Color::rgb(160, 160, 160),
            on_change: Rc::new(on_change),
            bounds: Rc::new(Cell::new(Rect::default())),
        }
    }

    pub fn step(mut self, value: f32) -> Self {
        self.step = if value == 0.5 { 0.5 } else { 1.0 };
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn colors(mut self, fill: Color, empty: Color) -> Self {
        self.fill = fill;
        self.empty = empty;
        self
    }
}

impl Widget for RawRating {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn style_state(&self) -> creamui_core::StyleState {
        creamui_core::StyleState::NORMAL.with_disabled(self.read_only)
    }

    fn focusable(&self) -> bool {
        !self.read_only
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.bounds.set(rect);
        false
    }

    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        if self.read_only {
            return None;
        }
        let callback = self.on_change.clone();
        let bounds = self.bounds.clone();
        let max = self.max;
        let step = self.step;
        Some(Rc::new(move |point| {
            let rect = bounds.get();
            if rect.width <= 0.0 {
                return;
            }
            callback(hit_value(point.x, rect, max, step));
        }))
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        if self.read_only {
            return None;
        }
        let value = self.value;
        let max = self.max as f32;
        let step = self.step;
        let callback = self.on_change.clone();
        Some(Rc::new(move |input| {
            let next = match input.key {
                Key::Left | Key::Down => value - step,
                Key::Right | Key::Up => value + step,
                Key::Home => 0.0,
                Key::End => max,
                _ => return,
            };
            callback(next.clamp(0.0, max));
        }))
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let width = rect.width / self.max as f32;
        for index in 0..self.max {
            let center = Point {
                x: rect.x + width * (index as f32 + 0.5),
                y: rect.y + rect.height / 2.0,
            };
            let radius = width.min(rect.height) * 0.42;
            let vertices: Vec<_> = (0..10)
                .map(|corner| {
                    let angle =
                        -std::f32::consts::FRAC_PI_2 + corner as f32 * std::f32::consts::PI / 5.0;
                    let distance = if corner % 2 == 0 {
                        radius
                    } else {
                        radius * 0.45
                    };
                    Point {
                        x: center.x + distance * angle.cos(),
                        y: center.y + distance * angle.sin(),
                    }
                })
                .collect();
            let amount = (self.value - index as f32).clamp(0.0, 1.0);
            if amount > 0.0 {
                let limit = center.x + radius * (amount * 2.0 - 1.0);
                for scan in 0..(radius * 2.0).ceil() as usize {
                    let y = center.y - radius + scan as f32 + 0.5;
                    let mut edges = Vec::new();
                    for edge in 0..10 {
                        let first = vertices[edge];
                        let second = vertices[(edge + 1) % 10];
                        if (first.y <= y && second.y > y) || (second.y <= y && first.y > y) {
                            edges.push(
                                first.x
                                    + (y - first.y) * (second.x - first.x) / (second.y - first.y),
                            );
                        }
                    }
                    edges.sort_by(|left, right| left.total_cmp(right));
                    for segment in edges.chunks_exact(2) {
                        let right = segment[1].min(limit);
                        if right > segment[0] {
                            painter.fill_rect(
                                Rect {
                                    x: segment[0],
                                    y: y - 0.5,
                                    width: right - segment[0],
                                    height: 1.0,
                                },
                                self.fill,
                                0.0,
                            );
                        }
                    }
                }
            }
            for edge in 0..10 {
                painter.stroke_line(
                    vertices[edge],
                    vertices[(edge + 1) % 10],
                    if amount >= 1.0 { self.fill } else { self.empty },
                    1.6,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rating_steps_from_pointer_and_keyboard() {
        let received = Rc::new(Cell::new(0.0));
        let sink = received.clone();
        let rating = RawRating::new(Style::default(), 2.5, 5, move |next| sink.set(next)).step(0.5);
        let rect = Rect {
            x: 10.0,
            y: 0.0,
            width: 100.0,
            height: 20.0,
        };
        rating.on_layout(rect);
        rating.on_click_at().unwrap()(Point { x: 21.0, y: 10.0 });
        assert_eq!(received.get(), 1.0);
        rating.on_key().unwrap()(KeyInput {
            key: Key::Right,
            modifiers: Default::default(),
        });
        assert_eq!(received.get(), 3.0);
        assert_eq!(hit_value(39.0, rect, 5, 0.5), 1.5);
    }

    #[test]
    fn read_only_rating_has_no_interaction() {
        let rating = RawRating::new(Style::default(), 4.0, 5, |_| {}).read_only(true);
        assert!(rating.on_click_at().is_none());
        assert!(rating.on_key().is_none());
    }
}
