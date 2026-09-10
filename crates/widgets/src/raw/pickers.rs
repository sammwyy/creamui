use super::*;

/// A small, dependency-free local date and time value used by
/// [`RawDateTimePicker`]. It deliberately has no timezone: applications can
/// convert it to their preferred calendar/time library at their boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
}

impl DateTime {
    pub fn new(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Self {
        Self {
            year,
            month: if month == 0 {
                1
            } else if month > 12 {
                12
            } else {
                month
            },
            day: if day == 0 { 1 } else { day },
            hour: if hour > 23 { 23 } else { hour },
            minute: if minute > 59 { 59 } else { minute },
        }
        .normalized()
    }

    pub fn days_in_month(year: i32, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
            2 => 28,
            _ => 31,
        }
    }

    pub fn normalized(mut self) -> Self {
        self.month = self.month.clamp(1, 12);
        self.day = self
            .day
            .clamp(1, Self::days_in_month(self.year, self.month));
        self.hour = self.hour.min(23);
        self.minute = self.minute.min(59);
        self
    }

    pub fn add_days(mut self, amount: i32) -> Self {
        self = self.normalized();
        if amount >= 0 {
            for _ in 0..amount {
                if self.day == Self::days_in_month(self.year, self.month) {
                    self.day = 1;
                    if self.month == 12 {
                        self.month = 1;
                        self.year += 1;
                    } else {
                        self.month += 1;
                    }
                } else {
                    self.day += 1;
                }
            }
        } else {
            for _ in 0..amount.unsigned_abs() {
                if self.day == 1 {
                    if self.month == 1 {
                        self.month = 12;
                        self.year -= 1;
                    } else {
                        self.month -= 1;
                    }
                    self.day = Self::days_in_month(self.year, self.month);
                } else {
                    self.day -= 1;
                }
            }
        }
        self
    }

    pub fn add_minutes(mut self, amount: i32) -> Self {
        self = self.normalized();
        let total = self.hour as i32 * 60 + self.minute as i32 + amount;
        let day_delta = total.div_euclid(24 * 60);
        let within_day = total.rem_euclid(24 * 60);
        self.hour = (within_day / 60) as u8;
        self.minute = (within_day % 60) as u8;
        self.add_days(day_delta)
    }

    pub fn display(self) -> String {
        let value = self.normalized();
        format!(
            "{:04}-{:02}-{:02}   {:02}:{:02}",
            value.year, value.month, value.day, value.hour, value.minute
        )
    }
}

impl Default for DateTime {
    fn default() -> Self {
        Self::new(2026, 1, 1, 9, 0)
    }
}

/// An unstyled compact date/time editor. Press the upper/lower half of the
/// date area to move one day, or the hour/minute area to move that part by
/// one. Arrow keys move one day (up/down) or one minute (left/right).
pub struct RawDateTimePicker {
    pub style: Style,
    pub value: DateTime,
    pub text_color: Color,
    pub background: Option<Color>,
    pub hover_background: Option<Color>,
    pub border_color: Color,
    pub border_width: f32,
    pub corner_radius: f32,
    pub focus_color: Option<Color>,
    pub on_change: Rc<dyn Fn(DateTime)>,
    pub on_activate: Option<Rc<dyn Fn()>>,
    pub trigger_only: bool,
    pub disabled: bool,
}

impl RawDateTimePicker {
    pub fn new(
        style: Style,
        value: DateTime,
        text_color: Color,
        border_color: Color,
        on_change: impl Fn(DateTime) + 'static,
    ) -> Self {
        Self {
            style,
            value: value.normalized(),
            text_color,
            background: None,
            hover_background: None,
            border_color,
            border_width: 1.0,
            corner_radius: 0.0,
            focus_color: None,
            on_change: Rc::new(on_change),
            on_activate: None,
            trigger_only: false,
            disabled: false,
        }
    }

    pub fn layout_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }
    pub fn hover_background(mut self, color: Color) -> Self {
        self.hover_background = Some(color);
        self
    }
    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border_color = color;
        self.border_width = width;
        self
    }
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }
    pub fn focus_color(mut self, color: Color) -> Self {
        self.focus_color = Some(color);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    /// Makes the field an activation trigger for an application-owned popup
    /// instead of editing values directly through its compact affordance.
    pub fn trigger_only(mut self, on_activate: impl Fn() + 'static) -> Self {
        self.on_activate = Some(Rc::new(on_activate));
        self.trigger_only = true;
        self
    }
}

impl Widget for RawDateTimePicker {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let background = if !self.disabled && painter.hovered(rect) {
            self.hover_background.or(self.background)
        } else {
            self.background
        };
        if let Some(color) = background {
            painter.fill_rect(rect, color, self.corner_radius);
        }
        painter.stroke_rect(
            rect,
            self.border_color,
            self.border_width,
            self.corner_radius,
        );
        let divider_x = rect.x + rect.width * 0.58;
        painter.stroke_line(
            Point {
                x: divider_x,
                y: rect.y + 8.,
            },
            Point {
                x: divider_x,
                y: rect.y + rect.height - 8.,
            },
            self.border_color,
            self.border_width,
        );
        painter.fill_text(
            Rect {
                x: rect.x + 10.,
                y: rect.y,
                width: (divider_x - rect.x - 16.).max(0.),
                height: rect.height,
            },
            &self.value.display()[..10],
            self.text_color,
            13.,
            TextAlign::Start,
        );
        painter.fill_text(
            Rect {
                x: divider_x + 10.,
                y: rect.y,
                width: (rect.x + rect.width - divider_x - 14.).max(0.),
                height: rect.height,
            },
            &self.value.display()[13..],
            self.text_color,
            13.,
            TextAlign::Start,
        );
    }
    fn focusable(&self) -> bool {
        !self.disabled
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        (!self.disabled && !self.trigger_only).then(|| {
            let value = self.value;
            let change = self.on_change.clone();
            Rc::new(move |input: KeyInput| match input.key {
                Key::Up => change(value.add_days(1)),
                Key::Down => change(value.add_days(-1)),
                Key::Left => change(value.add_minutes(-1)),
                Key::Right => change(value.add_minutes(1)),
                _ => {}
            }) as Rc<dyn Fn(KeyInput)>
        })
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        (!self.disabled && !self.trigger_only).then(|| {
            let value = self.value;
            let change = self.on_change.clone();
            Rc::new(move |local: Point, rect: Rect| {
                let up = local.y < rect.height / 2.;
                let direction = if up { 1 } else { -1 };
                if local.x < rect.width * 0.58 {
                    change(value.add_days(direction));
                } else if local.x < rect.width * 0.79 {
                    change(value.add_minutes(direction * 60));
                } else {
                    change(value.add_minutes(direction));
                }
            }) as Rc<dyn Fn(Point, Rect)>
        })
    }
    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, _: bool) {
        painter.stroke_rect(
            rect,
            self.focus_color.unwrap_or(self.border_color),
            2.,
            self.corner_radius,
        );
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        (!self.disabled && self.trigger_only)
            .then(|| self.on_activate.clone().expect("trigger has callback"))
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }
}

fn rgb_to_hsv(color: Color) -> (f32, f32, f32) {
    let r = color.r as f32 / 255.;
    let g = color.g as f32 / 255.;
    let b = color.b as f32 / 255.;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0. {
        0.
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.) / 6.
    } else if max == g {
        ((b - r) / delta + 2.) / 6.
    } else {
        ((r - g) / delta + 4.) / 6.
    };
    (hue, if max == 0. { 0. } else { delta / max }, max)
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32, alpha: u8) -> Color {
    let h = hue.rem_euclid(1.) * 6.;
    let c = value * saturation;
    let x = c * (1. - (h.rem_euclid(2.) - 1.).abs());
    let (r, g, b) = match h as u8 {
        0 => (c, x, 0.),
        1 => (x, c, 0.),
        2 => (0., c, x),
        3 => (0., x, c),
        4 => (x, 0., c),
        _ => (c, 0., x),
    };
    let m = value - c;
    Color::rgba(
        ((r + m) * 255.).round() as u8,
        ((g + m) * 255.).round() as u8,
        ((b + m) * 255.).round() as u8,
        alpha,
    )
}

/// An unstyled HSV color field. Drag in the large square to choose saturation
/// and brightness; drag the narrow strip at right to choose hue.
pub struct RawColorPicker {
    pub style: Style,
    pub value: Color,
    pub background: Option<Color>,
    pub border_color: Color,
    pub border_width: f32,
    pub handle_color: Color,
    pub corner_radius: f32,
    pub focus_color: Option<Color>,
    pub on_change: Rc<dyn Fn(Color)>,
    pub disabled: bool,
}

impl RawColorPicker {
    pub fn new(
        style: Style,
        value: Color,
        border_color: Color,
        handle_color: Color,
        on_change: impl Fn(Color) + 'static,
    ) -> Self {
        Self {
            style,
            value,
            background: None,
            border_color,
            border_width: 1.,
            handle_color,
            corner_radius: 0.,
            focus_color: None,
            on_change: Rc::new(on_change),
            disabled: false,
        }
    }
    pub fn layout_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }
    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border_color = color;
        self.border_width = width;
        self
    }
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }
    pub fn focus_color(mut self, color: Color) -> Self {
        self.focus_color = Some(color);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Widget for RawColorPicker {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(color) = self.background {
            painter.fill_rect(rect, color, self.corner_radius);
        }
        let (hue, saturation, value) = rgb_to_hsv(self.value);
        let hue_width = 16.;
        let field_width = (rect.width - hue_width - 8.).max(1.);
        let steps = 14.;
        for y in 0..14 {
            for x in 0..14 {
                let x0 = rect.x + x as f32 * field_width / steps;
                let y0 = rect.y + y as f32 * rect.height / steps;
                painter.fill_rect(
                    Rect {
                        x: x0,
                        y: y0,
                        width: field_width / steps + 1.,
                        height: rect.height / steps + 1.,
                    },
                    hsv_to_rgb(
                        hue,
                        x as f32 / (steps - 1.),
                        1. - y as f32 / (steps - 1.),
                        self.value.a,
                    ),
                    0.,
                );
            }
        }
        let hue_x = rect.x + field_width + 8.;
        for y in 0..14 {
            painter.fill_rect(
                Rect {
                    x: hue_x,
                    y: rect.y + y as f32 * rect.height / steps,
                    width: hue_width,
                    height: rect.height / steps + 1.,
                },
                hsv_to_rgb(y as f32 / steps, 1., 1., self.value.a),
                2.,
            );
        }
        painter.stroke_rect(
            rect,
            self.border_color,
            self.border_width,
            self.corner_radius,
        );
        painter.stroke_rect(
            Rect {
                x: rect.x + saturation * field_width - 4.,
                y: rect.y + (1. - value) * rect.height - 4.,
                width: 8.,
                height: 8.,
            },
            self.handle_color,
            2.,
            4.,
        );
        painter.stroke_rect(
            Rect {
                x: hue_x - 2.,
                y: rect.y + hue * rect.height - 2.,
                width: hue_width + 4.,
                height: 4.,
            },
            self.handle_color,
            2.,
            1.,
        );
    }
    fn focusable(&self) -> bool {
        !self.disabled
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        (!self.disabled).then(|| {
            let (h, s, v) = rgb_to_hsv(self.value);
            let change = self.on_change.clone();
            let alpha = self.value.a;
            Rc::new(move |input: KeyInput| match input.key {
                Key::Left => change(hsv_to_rgb(h - 0.02, s, v, alpha)),
                Key::Right => change(hsv_to_rgb(h + 0.02, s, v, alpha)),
                Key::Up => change(hsv_to_rgb(h, s, (v + 0.04).min(1.), alpha)),
                Key::Down => change(hsv_to_rgb(h, s, (v - 0.04).max(0.), alpha)),
                _ => {}
            }) as Rc<dyn Fn(KeyInput)>
        })
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        (!self.disabled).then(|| {
            let (hue, s, v) = rgb_to_hsv(self.value);
            let change = self.on_change.clone();
            let alpha = self.value.a;
            Rc::new(move |local: Point, rect: Rect| {
                let field_width = (rect.width - 24.).max(1.);
                if local.x >= field_width + 8. {
                    change(hsv_to_rgb(
                        (local.y / rect.height.max(1.)).clamp(0., 1.),
                        s,
                        v,
                        alpha,
                    ));
                } else {
                    change(hsv_to_rgb(
                        hue,
                        (local.x / field_width).clamp(0., 1.),
                        (1. - local.y / rect.height.max(1.)).clamp(0., 1.),
                        alpha,
                    ));
                }
            }) as Rc<dyn Fn(Point, Rect)>
        })
    }
    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, _: bool) {
        painter.stroke_rect(
            rect,
            self.focus_color.unwrap_or(self.handle_color),
            2.,
            self.corner_radius,
        );
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }
}

/// An unstyled file-selection trigger. It only renders and reports activation;
/// the application decides whether that means a native dialog, a remote asset
/// browser, or another file source.
pub struct RawFilePicker {
    pub style: Style,
    pub value: String,
    pub placeholder: String,
    pub text_color: Color,
    pub placeholder_color: Color,
    pub background: Option<Color>,
    pub hover_background: Option<Color>,
    pub border_color: Color,
    pub border_width: f32,
    pub corner_radius: f32,
    pub focus_color: Option<Color>,
    pub on_activate: Rc<dyn Fn()>,
    pub disabled: bool,
}

impl RawFilePicker {
    pub fn new(
        style: Style,
        value: impl Into<String>,
        text_color: Color,
        placeholder_color: Color,
        border_color: Color,
        on_activate: impl Fn() + 'static,
    ) -> Self {
        Self {
            style,
            value: value.into(),
            placeholder: "Choose a file…".into(),
            text_color,
            placeholder_color,
            background: None,
            hover_background: None,
            border_color,
            border_width: 1.,
            corner_radius: 0.,
            focus_color: None,
            on_activate: Rc::new(on_activate),
            disabled: false,
        }
    }
    pub fn layout_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }
    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }
    pub fn hover_background(mut self, color: Color) -> Self {
        self.hover_background = Some(color);
        self
    }
    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border_color = color;
        self.border_width = width;
        self
    }
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }
    pub fn focus_color(mut self, color: Color) -> Self {
        self.focus_color = Some(color);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Widget for RawFilePicker {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let background = if !self.disabled && painter.hovered(rect) {
            self.hover_background.or(self.background)
        } else {
            self.background
        };
        if let Some(color) = background {
            painter.fill_rect(rect, color, self.corner_radius);
        }
        painter.stroke_rect(
            rect,
            self.border_color,
            self.border_width,
            self.corner_radius,
        );
        let label = if self.value.is_empty() {
            &self.placeholder
        } else {
            &self.value
        };
        painter.fill_text(
            Rect {
                x: rect.x + 11.,
                y: rect.y,
                width: (rect.width - 84.).max(0.),
                height: rect.height,
            },
            label,
            if self.value.is_empty() {
                self.placeholder_color
            } else {
                self.text_color
            },
            13.,
            TextAlign::Start,
        );
        painter.stroke_rect(
            Rect {
                x: rect.x + rect.width - 63.,
                y: rect.y + 7.,
                width: 55.,
                height: (rect.height - 14.).max(0.),
            },
            self.border_color,
            self.border_width,
            (self.corner_radius - 2.).max(0.),
        );
        painter.fill_text(
            Rect {
                x: rect.x + rect.width - 63.,
                y: rect.y,
                width: 55.,
                height: rect.height,
            },
            "Browse",
            self.text_color,
            12.,
            TextAlign::Center,
        );
    }
    fn focusable(&self) -> bool {
        !self.disabled
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        (!self.disabled).then(|| activate_on_key(self.on_activate.clone()))
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        (!self.disabled).then(|| self.on_activate.clone())
    }
    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, _: bool) {
        painter.stroke_rect(
            rect,
            self.focus_color.unwrap_or(self.border_color),
            2.,
            self.corner_radius,
        );
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_reactive::Signal;

    #[test]
    fn date_time_normalizes_and_crosses_calendar_boundaries() {
        let leap_day = DateTime::new(2024, 2, 29, 23, 59).add_minutes(1);
        assert_eq!(leap_day, DateTime::new(2024, 3, 1, 0, 0));
        assert_eq!(DateTime::new(2025, 3, 1, 0, 0).add_days(-1).day, 28);
    }

    #[test]
    fn color_picker_drag_reports_controlled_color() {
        let value = Signal::new(Color::rgb(255, 0, 0));
        let set = value.clone();
        let picker = RawColorPicker::new(
            Style::default(),
            value.get(),
            Color::rgb(0, 0, 0),
            Color::rgb(255, 255, 255),
            move |next| set.set(next),
        );
        picker.on_drag().unwrap()(
            Point { x: 0.0, y: 156.0 },
            Rect {
                x: 0.0,
                y: 0.0,
                width: 220.0,
                height: 156.0,
            },
        );
        assert_eq!(value.get(), Color::rgb(0, 0, 0));
    }
}
