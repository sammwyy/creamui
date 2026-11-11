use super::*;
use creamui_core::Modifiers;
use creamui_reactive::Signal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotKind {
    Line,
    Bar,
    Area,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotScale {
    pub min: f32,
    pub max: f32,
}

impl PlotScale {
    pub fn new(min: f32, max: f32) -> Self {
        if !min.is_finite() || !max.is_finite() {
            return Self { min: 0.0, max: 1.0 };
        }
        Self {
            min,
            max: if max <= min { min + 1.0 } else { max },
        }
    }

    fn position(self, value: f32) -> f32 {
        (value - self.min) / (self.max - self.min).max(f32::EPSILON)
    }
}

#[derive(Clone)]
pub struct PlotSeries {
    pub label: String,
    pub kind: PlotKind,
    pub color: Color,
    pub points: Vec<(f32, f32)>,
}

impl PlotSeries {
    pub fn new(
        label: impl Into<String>,
        kind: PlotKind,
        color: Color,
        points: Vec<(f32, f32)>,
    ) -> Self {
        Self {
            label: label.into(),
            kind,
            color,
            points,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotHit {
    pub series: usize,
    pub point: usize,
    pub value: (f32, f32),
}

#[derive(Clone)]
pub struct PlotController {
    hovered: Signal<Option<PlotHit>>,
}

impl PlotController {
    pub fn new() -> Self {
        Self {
            hovered: Signal::new(None),
        }
    }

    pub fn hovered(&self) -> Option<PlotHit> {
        self.hovered.get()
    }
}

impl Default for PlotController {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RawPlot {
    pub style: creamui_core::Style,
    pub series: Vec<PlotSeries>,
    pub x_scale: PlotScale,
    pub y_scale: PlotScale,
    pub axis_color: Color,
    pub grid_color: Color,
    pub label_color: Color,
    controller: PlotController,
    rect: Rc<Cell<Rect>>,
}

impl RawPlot {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        controller: &PlotController,
        series: Vec<PlotSeries>,
    ) -> Self {
        let limits = series
            .iter()
            .flat_map(|series| series.points.iter().copied())
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .fold(None, |limits: Option<(f32, f32, f32, f32)>, (x, y)| {
                Some(match limits {
                    Some((xmin, xmax, ymin, ymax)) => {
                        (xmin.min(x), xmax.max(x), ymin.min(y), ymax.max(y))
                    }
                    None => (x, x, y.min(0.0), y.max(0.0)),
                })
            });
        let (xmin, xmax, ymin, ymax) = limits.unwrap_or((0.0, 1.0, 0.0, 1.0));
        let _ = controller.hovered();
        Self {
            style: style.into(),
            series,
            x_scale: PlotScale::new(xmin, xmax),
            y_scale: PlotScale::new(ymin, ymax),
            axis_color: Color::rgb(120, 120, 120),
            grid_color: Color::rgba(120, 120, 120, 70),
            label_color: Color::rgb(120, 120, 120),
            controller: controller.clone(),
            rect: Rc::new(Cell::new(Rect::default())),
        }
    }

    pub fn scales(mut self, x: PlotScale, y: PlotScale) -> Self {
        self.x_scale = x;
        self.y_scale = y;
        self
    }

    fn bounds(rect: Rect) -> Rect {
        Rect {
            x: rect.x + 38.0,
            y: rect.y + 12.0,
            width: (rect.width - 50.0).max(0.0),
            height: (rect.height - 36.0).max(0.0),
        }
    }

    fn screen(&self, point: (f32, f32), bounds: Rect) -> Point {
        Self::screen_at(point, bounds, self.x_scale, self.y_scale)
    }

    fn screen_at(point: (f32, f32), bounds: Rect, x_scale: PlotScale, y_scale: PlotScale) -> Point {
        Point {
            x: bounds.x + bounds.width * x_scale.position(point.0),
            y: bounds.y + bounds.height * (1.0 - y_scale.position(point.1)),
        }
    }

    fn bar_width(bounds: Rect, count: usize) -> f32 {
        (bounds.width / count.max(1) as f32 * 0.55).clamp(3.0, 26.0)
    }

    fn clip_segment(from: Point, to: Point, bounds: Rect) -> Option<(Point, Point)> {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let mut start: f32 = 0.0;
        let mut end: f32 = 1.0;
        for (p, q) in [
            (-dx, from.x - bounds.x),
            (dx, bounds.x + bounds.width - from.x),
            (-dy, from.y - bounds.y),
            (dy, bounds.y + bounds.height - from.y),
        ] {
            if p.abs() < f32::EPSILON {
                if q < 0.0 {
                    return None;
                }
            } else {
                let ratio = q / p;
                if p < 0.0 {
                    start = start.max(ratio);
                } else {
                    end = end.min(ratio);
                }
            }
        }
        (start <= end).then_some((
            Point {
                x: from.x + dx * start,
                y: from.y + dy * start,
            },
            Point {
                x: from.x + dx * end,
                y: from.y + dy * end,
            },
        ))
    }

    pub fn hit(&self, point: Point, rect: Rect) -> Option<PlotHit> {
        Self::nearest_hit(&self.series, self.x_scale, self.y_scale, point, rect)
    }

    fn nearest_hit(
        series: &[PlotSeries],
        x_scale: PlotScale,
        y_scale: PlotScale,
        point: Point,
        rect: Rect,
    ) -> Option<PlotHit> {
        let bounds = Self::bounds(rect);
        if !bounds.contains(point) {
            return None;
        }
        series
            .iter()
            .enumerate()
            .flat_map(|(series_index, series)| {
                series
                    .points
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(move |(_, point)| point.0.is_finite() && point.1.is_finite())
                    .map(move |(index, value)| (series_index, index, value))
            })
            .filter_map(|(series_index, index, value)| {
                let mut position = Self::screen_at(value, bounds, x_scale, y_scale);
                if series[series_index].kind == PlotKind::Bar {
                    let width = Self::bar_width(bounds, series[series_index].points.len());
                    if position.x < bounds.x || position.x > bounds.x + bounds.width {
                        return None;
                    }
                    position.x = position.x.clamp(
                        bounds.x + width / 2.0,
                        bounds.x + bounds.width - width / 2.0,
                    );
                }
                let distance = (position.x - point.x).hypot(position.y - point.y);
                (distance <= 12.0).then_some((
                    PlotHit {
                        series: series_index,
                        point: index,
                        value,
                    },
                    distance,
                ))
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(hit, _)| hit)
    }
}

impl Widget for RawPlot {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn reports_layout(&self) -> bool {
        true
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.rect.set(rect);
        false
    }

    fn on_pointer_move(&self) -> Option<Rc<dyn Fn(Point, Modifiers)>> {
        let rect = self.rect.clone();
        let controller = self.controller.clone();
        let series = Rc::new(self.series.clone());
        let x_scale = self.x_scale;
        let y_scale = self.y_scale;
        Some(Rc::new(move |local, _| {
            let rect = rect.get();
            let hit = RawPlot::nearest_hit(
                &series,
                x_scale,
                y_scale,
                Point {
                    x: rect.x + local.x,
                    y: rect.y + local.y,
                },
                rect,
            );
            controller.hovered.set_if_changed(hit);
        }))
    }

    fn on_hover(&self) -> Option<Rc<dyn Fn(bool)>> {
        let controller = self.controller.clone();
        Some(Rc::new(move |hovered| {
            if !hovered {
                controller.hovered.set_if_changed(None);
            }
        }))
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let bounds = Self::bounds(rect);
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return;
        }
        for index in 0..=4 {
            let y = bounds.y + bounds.height * index as f32 / 4.0;
            painter.stroke_line(
                Point { x: bounds.x, y },
                Point {
                    x: bounds.x + bounds.width,
                    y,
                },
                self.grid_color,
                1.0,
            );
        }
        painter.stroke_line(
            Point {
                x: bounds.x,
                y: bounds.y,
            },
            Point {
                x: bounds.x,
                y: bounds.y + bounds.height,
            },
            self.axis_color,
            1.0,
        );
        painter.push_clip(bounds);
        for series in &self.series {
            let points: Vec<_> = series
                .points
                .iter()
                .copied()
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .map(|value| self.screen(value, bounds))
                .collect();
            match series.kind {
                PlotKind::Line | PlotKind::Area => {
                    for pair in points.windows(2) {
                        let Some((from, to)) = Self::clip_segment(pair[0], pair[1], bounds) else {
                            continue;
                        };
                        if series.kind == PlotKind::Area {
                            let steps = ((to.x - from.x).abs() / 3.0).ceil() as usize;
                            let fill =
                                Color::rgba(series.color.r, series.color.g, series.color.b, 50);
                            for step in 0..=steps {
                                let t = step as f32 / steps.max(1) as f32;
                                let x = from.x + (to.x - from.x) * t;
                                let y = from.y + (to.y - from.y) * t;
                                painter.fill_rect(
                                    Rect {
                                        x,
                                        y,
                                        width: 3.0,
                                        height: (bounds.y + bounds.height - y).max(0.0),
                                    },
                                    fill,
                                    0.0,
                                );
                            }
                        }
                        painter.stroke_line(from, to, series.color, 2.0);
                    }
                }
                PlotKind::Bar => {
                    let width = Self::bar_width(bounds, series.points.len());
                    for point in points {
                        if point.x < bounds.x || point.x > bounds.x + bounds.width {
                            continue;
                        }
                        painter.fill_rect(
                            Rect {
                                x: point.x.clamp(
                                    bounds.x + width / 2.0,
                                    bounds.x + bounds.width - width / 2.0,
                                ) - width / 2.0,
                                y: point.y.max(bounds.y),
                                width,
                                height: (bounds.y + bounds.height - point.y.max(bounds.y)).max(0.0),
                            },
                            series.color,
                            2.0,
                        );
                    }
                }
            }
        }
        if let Some(hit) = self.controller.hovered() {
            if let Some(value) = self
                .series
                .get(hit.series)
                .and_then(|series| series.points.get(hit.point))
            {
                let mut point = self.screen(*value, bounds);
                if self.series[hit.series].kind == PlotKind::Bar {
                    let width = Self::bar_width(bounds, self.series[hit.series].points.len());
                    point.x = point.x.clamp(
                        bounds.x + width / 2.0,
                        bounds.x + bounds.width - width / 2.0,
                    );
                }
                painter.fill_rect(
                    Rect {
                        x: point.x - 4.0,
                        y: point.y - 4.0,
                        width: 8.0,
                        height: 8.0,
                    },
                    self.series[hit.series].color,
                    4.0,
                );
            }
        }
        painter.pop_clip();
        painter.fill_text(
            Rect {
                x: rect.x,
                y: bounds.y,
                width: 35.0,
                height: 15.0,
            },
            &format!("{:.0}", self.y_scale.max),
            self.label_color,
            11.0,
            TextAlign::End,
        );
        painter.fill_text(
            Rect {
                x: rect.x,
                y: bounds.y + bounds.height - 15.0,
                width: 35.0,
                height: 15.0,
            },
            &format!("{:.0}", self.y_scale.min),
            self.label_color,
            11.0,
            TextAlign::End,
        );
        painter.fill_text(
            Rect {
                x: bounds.x,
                y: bounds.y + bounds.height + 4.0,
                width: 40.0,
                height: 16.0,
            },
            &format!("{:.0}", self.x_scale.min),
            self.label_color,
            11.0,
            TextAlign::Start,
        );
        painter.fill_text(
            Rect {
                x: bounds.x + bounds.width - 40.0,
                y: bounds.y + bounds.height + 4.0,
                width: 40.0,
                height: 16.0,
            },
            &format!("{:.0}", self.x_scale.max),
            self.label_color,
            11.0,
            TextAlign::End,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_hit_respects_explicit_scales_and_plot_bounds() {
        let controller = PlotController::new();
        let plot = RawPlot::new(
            Style::default(),
            &controller,
            vec![PlotSeries::new(
                "A",
                PlotKind::Line,
                Color::rgb(255, 0, 0),
                vec![(0.0, 0.0), (10.0, 20.0)],
            )],
        )
        .scales(PlotScale::new(0.0, 10.0), PlotScale::new(0.0, 20.0));
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 250.0,
            height: 150.0,
        };
        assert_eq!(
            plot.hit(Point { x: 238.0, y: 12.0 }, rect).unwrap().point,
            1
        );
        assert!(plot.hit(Point { x: 5.0, y: 5.0 }, rect).is_none());
    }

    #[test]
    fn line_segments_clip_to_plot_bounds() {
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        };
        let (from, to) = RawPlot::clip_segment(
            Point {
                x: -1_000_000.0,
                y: 25.0,
            },
            Point {
                x: 1_000_000.0,
                y: 25.0,
            },
            bounds,
        )
        .unwrap();
        assert!((from.x - 0.0).abs() <= 0.01);
        assert!((to.x - 100.0).abs() <= 0.01);
        assert!(RawPlot::clip_segment(
            Point { x: -10.0, y: 4.0 },
            Point { x: -5.0, y: 8.0 },
            bounds,
        )
        .is_none());
    }
}
