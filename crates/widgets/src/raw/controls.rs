use super::*;
/// An unstyled checkbox: a fixed-size clickable box, filled when `checked`
/// and outlined otherwise. The caller owns the checked state (typically a
/// `bool` `Signal`) and toggles it from `on_click` — this widget has no
/// state of its own, same as every other widget (see `creamui_core::Widget`).
pub struct RawCheckbox {
    pub style: Style,
    pub box_size: f32,
    pub checked: bool,
    pub fill_color: Color,
    pub background: Option<Color>,
    pub hover_background: Option<Color>,
    pub pressed_background: Option<Color>,
    pub border_color: Color,
    pub border_width: f32,
    pub check_color: Color,
    pub focus_color: Option<Color>,
    pub corner_radius: f32,
    pub on_click: Rc<dyn Fn()>,
    pub disabled: bool,
}

impl RawCheckbox {
    pub fn new(
        box_size: f32,
        checked: bool,
        fill_color: Color,
        border_color: Color,
        on_click: impl Fn() + 'static,
    ) -> Self {
        RawCheckbox {
            style: Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(box_size),
                    height: creamui_core::layout::Dimension::Length(box_size),
                },
                ..Default::default()
            },
            box_size,
            checked,
            fill_color,
            background: None,
            hover_background: None,
            pressed_background: None,
            border_color,
            border_width: 1.5,
            check_color: Color::rgb(255, 255, 255),
            focus_color: None,
            corner_radius: 0.0,
            on_click: Rc::new(on_click),
            disabled: false,
        }
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
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

    pub fn pressed_background(mut self, color: Color) -> Self {
        self.pressed_background = Some(color);
        self
    }

    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border_color = color;
        self.border_width = width;
        self
    }

    pub fn check_color(mut self, color: Color) -> Self {
        self.check_color = color;
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

impl Widget for RawCheckbox {
    fn focusable(&self) -> bool {
        !self.disabled
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        (!self.disabled).then(|| activate_on_key(self.on_click.clone()))
    }
    fn paint_focused_overlay(&self, p: &mut dyn Painter, r: Rect, _: bool) {
        p.stroke_rect(
            Rect {
                x: r.x - 3.,
                y: r.y - 3.,
                width: r.width + 6.,
                height: r.height + 6.,
            },
            self.focus_color.unwrap_or(self.fill_color),
            2.,
            self.corner_radius + 2.,
        );
    }
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let base_background = if self.checked {
            Some(self.fill_color)
        } else {
            self.background
        };
        let background = if !self.disabled && painter.pressed(rect) {
            self.pressed_background
                .or(self.hover_background)
                .or(base_background)
        } else if !self.disabled && painter.hovered(rect) {
            self.hover_background.or(base_background)
        } else {
            base_background
        };
        if let Some(color) = background {
            painter.fill_rect(rect, color, self.corner_radius);
        }
        if self.checked {
            let p = |x: f32, y: f32| Point {
                x: rect.x + rect.width * x,
                y: rect.y + rect.height * y,
            };
            painter.stroke_line(p(0.25, 0.5), p(0.43, 0.68), self.check_color, 1.8);
            painter.stroke_line(p(0.43, 0.68), p(0.76, 0.32), self.check_color, 1.8);
        } else {
            painter.stroke_rect(
                rect,
                self.border_color,
                self.border_width,
                self.corner_radius,
            );
        }
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        (!self.disabled).then(|| self.on_click.clone())
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }
}

/// Headless iOS/macOS-style boolean switch: a pill track with a sliding thumb.
pub struct RawSwitch {
    pub style: Style,
    pub checked: bool,
    pub on_color: Color,
    pub off_color: Color,
    pub thumb_color: Color,
    pub hover_on_color: Option<Color>,
    pub hover_off_color: Option<Color>,
    pub pressed_on_color: Option<Color>,
    pub pressed_off_color: Option<Color>,
    pub focus_color: Option<Color>,
    pub track_radius: Option<f32>,
    pub thumb_radius: Option<f32>,
    pub thumb_inset: f32,
    pub on_click: Rc<dyn Fn()>,
    pub disabled: bool,
}
impl RawSwitch {
    pub fn new(
        checked: bool,
        on_color: Color,
        off_color: Color,
        thumb_color: Color,
        on_click: impl Fn() + 'static,
    ) -> Self {
        Self {
            style: Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(42.0),
                    height: creamui_core::layout::Dimension::Length(24.0),
                },
                ..Default::default()
            },
            checked,
            on_color,
            off_color,
            thumb_color,
            hover_on_color: None,
            hover_off_color: None,
            pressed_on_color: None,
            pressed_off_color: None,
            focus_color: None,
            track_radius: None,
            thumb_radius: None,
            thumb_inset: 2.0,
            on_click: Rc::new(on_click),
            disabled: false,
        }
    }

    pub fn layout_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn hover_colors(mut self, on: Color, off: Color) -> Self {
        self.hover_on_color = Some(on);
        self.hover_off_color = Some(off);
        self
    }

    pub fn pressed_colors(mut self, on: Color, off: Color) -> Self {
        self.pressed_on_color = Some(on);
        self.pressed_off_color = Some(off);
        self
    }

    pub fn radii(mut self, track: f32, thumb: f32) -> Self {
        self.track_radius = Some(track);
        self.thumb_radius = Some(thumb);
        self
    }

    pub fn thumb_inset(mut self, inset: f32) -> Self {
        self.thumb_inset = inset.max(0.0);
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
impl Widget for RawSwitch {
    fn focusable(&self) -> bool {
        !self.disabled
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        (!self.disabled).then(|| activate_on_key(self.on_click.clone()))
    }
    fn paint_focused_overlay(&self, p: &mut dyn Painter, r: Rect, _: bool) {
        p.stroke_rect(
            Rect {
                x: r.x - 3.,
                y: r.y - 3.,
                width: r.width + 6.,
                height: r.height + 6.,
            },
            self.focus_color.unwrap_or(self.on_color),
            2.,
            r.height / 2. + 3.,
        );
    }
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let base = if self.checked {
            self.on_color
        } else {
            self.off_color
        };
        let hover = if self.checked {
            self.hover_on_color
        } else {
            self.hover_off_color
        };
        let pressed = if self.checked {
            self.pressed_on_color
        } else {
            self.pressed_off_color
        };
        let track_color = if !self.disabled && painter.pressed(rect) {
            pressed.or(hover).unwrap_or(base)
        } else if !self.disabled && painter.hovered(rect) {
            hover.unwrap_or(base)
        } else {
            base
        };
        let d = (rect.height - self.thumb_inset * 2.0).max(0.0);
        painter.fill_rect(
            rect,
            track_color,
            self.track_radius.unwrap_or(rect.height / 2.0),
        );
        painter.fill_rect(
            Rect {
                x: if self.checked {
                    rect.x + rect.width - d - self.thumb_inset
                } else {
                    rect.x + self.thumb_inset
                },
                y: rect.y + self.thumb_inset,
                width: d,
                height: d,
            },
            self.thumb_color,
            self.thumb_radius.unwrap_or(d / 2.0),
        );
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        (!self.disabled).then(|| self.on_click.clone())
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }
}

/// An unstyled horizontal slider: drag (or click) anywhere along its track
/// to set a `0.0..=1.0` value. The caller owns the value (typically an
/// `f32` `Signal`) and updates it from `on_change` — same pattern as every
/// other interactive widget here.
pub struct RawSlider {
    pub style: Style,
    pub value: f32,
    pub track_color: Color,
    pub fill_color: Color,
    pub handle_color: Color,
    pub hover_handle_color: Option<Color>,
    pub pressed_handle_color: Option<Color>,
    pub track_height: f32,
    pub track_radius: Option<f32>,
    pub handle_size: f32,
    pub handle_radius: Option<f32>,
    pub focus_color: Option<Color>,
    pub on_change: Rc<dyn Fn(f32)>,
    pub disabled: bool,
}

impl RawSlider {
    pub fn new(
        style: Style,
        value: f32,
        track_color: Color,
        fill_color: Color,
        handle_color: Color,
        on_change: impl Fn(f32) + 'static,
    ) -> Self {
        RawSlider {
            style,
            value: value.clamp(0.0, 1.0),
            track_color,
            fill_color,
            handle_color,
            hover_handle_color: None,
            pressed_handle_color: None,
            track_height: 4.0,
            track_radius: None,
            handle_size: 16.0,
            handle_radius: None,
            focus_color: None,
            on_change: Rc::new(on_change),
            disabled: false,
        }
    }

    pub fn layout_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn track(mut self, height: f32, radius: f32) -> Self {
        self.track_height = height.max(0.0);
        self.track_radius = Some(radius.max(0.0));
        self
    }

    pub fn handle(mut self, size: f32, radius: f32) -> Self {
        self.handle_size = size.max(0.0);
        self.handle_radius = Some(radius.max(0.0));
        self
    }

    pub fn hover_handle_color(mut self, color: Color) -> Self {
        self.hover_handle_color = Some(color);
        self
    }

    pub fn pressed_handle_color(mut self, color: Color) -> Self {
        self.pressed_handle_color = Some(color);
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

impl Widget for RawSlider {
    fn focusable(&self) -> bool {
        !self.disabled
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        if self.disabled {
            return None;
        }
        let value = self.value;
        let change = self.on_change.clone();
        Some(Rc::new(move |input| {
            let next = match input.key {
                Key::Left | Key::Down => value - 0.05,
                Key::Right | Key::Up => value + 0.05,
                Key::Home => 0.,
                Key::End => 1.,
                _ => return,
            };
            change(next.clamp(0., 1.));
        }))
    }
    fn paint_focused_overlay(&self, p: &mut dyn Painter, r: Rect, _: bool) {
        p.stroke_rect(r, self.focus_color.unwrap_or(self.fill_color), 1., 5.);
    }
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let track_y = rect.y + (rect.height - self.track_height) / 2.0;
        let usable_width = (rect.width - self.handle_size).max(0.0);
        let handle_center_x = rect.x + self.handle_size / 2.0 + usable_width * self.value;

        painter.fill_rect(
            Rect {
                x: rect.x,
                y: track_y,
                width: rect.width,
                height: self.track_height,
            },
            self.track_color,
            self.track_radius.unwrap_or(self.track_height / 2.0),
        );
        painter.fill_rect(
            Rect {
                x: rect.x,
                y: track_y,
                width: (handle_center_x - rect.x).max(0.0),
                height: self.track_height,
            },
            self.fill_color,
            self.track_radius.unwrap_or(self.track_height / 2.0),
        );
        painter.fill_rect(
            Rect {
                x: handle_center_x - self.handle_size / 2.0,
                y: rect.y + (rect.height - self.handle_size) / 2.0,
                width: self.handle_size,
                height: self.handle_size,
            },
            if !self.disabled && painter.pressed(rect) {
                self.pressed_handle_color
                    .or(self.hover_handle_color)
                    .unwrap_or(self.handle_color)
            } else if !self.disabled && painter.hovered(rect) {
                self.hover_handle_color.unwrap_or(self.handle_color)
            } else {
                self.handle_color
            },
            self.handle_radius.unwrap_or(self.handle_size / 2.0),
        );
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        if self.disabled {
            return None;
        }
        let on_change = self.on_change.clone();
        let handle_size = self.handle_size;
        Some(Rc::new(move |local: Point, rect: Rect| {
            let usable_width = (rect.width - handle_size).max(1.0);
            let fraction = ((local.x - handle_size / 2.0) / usable_width).clamp(0.0, 1.0);
            on_change(fraction);
        }))
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }
}
