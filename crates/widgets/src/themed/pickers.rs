use super::*;
use crate::layout::{column, fixed, padding, row};
use crate::raw::{DateTime, RawColorPicker, RawFilePicker};
use creamui_core::layout::{AlignItems, Dimension, LengthPercentageAuto, Position};
use creamui_core::Key;
use std::path::PathBuf;

fn popup_style(width: f32, top: f32, gap: f32, padding_amount: f32) -> Style {
    padding(
        Style {
            position: Position::Absolute,
            inset: creamui_core::layout::Rect {
                left: LengthPercentageAuto::Length(0.),
                right: LengthPercentageAuto::Auto,
                top: LengthPercentageAuto::Length(top),
                bottom: LengthPercentageAuto::Auto,
            },
            size: creamui_core::layout::Size {
                width: Dimension::Length(width),
                height: Dimension::Auto,
            },
            ..column(gap)
        },
        padding_amount,
    )
}

fn popup_button(label: impl Into<String>, width: f32, on_click: impl Fn() + 'static) -> RawButton {
    let theme = use_theme();
    RawButton::new(
        Style {
            size: fixed(width, 28.),
            align_items: Some(AlignItems::Center),
            justify_content: Some(JustifyContent::Center),
            flex_shrink: 0.,
            ..Default::default()
        },
        on_click,
    )
    .background(theme.surface_hover)
    .corner_radius(theme.menu_item_radius)
    .border(theme.border, 1.)
    .child(Box::new(
        RawText::new(label, theme.text_primary, 12.).align(TextAlign::Center),
    ))
}

fn month_shift(value: DateTime, amount: i32) -> DateTime {
    let month_index = value.month as i32 - 1 + amount;
    let year = value.year + month_index.div_euclid(12);
    let month = (month_index.rem_euclid(12) + 1) as u8;
    DateTime::new(
        year,
        month,
        value.day.min(DateTime::days_in_month(year, month)),
        value.hour,
        value.minute,
    )
}

fn weekday(year: i32, month: u8, day: u8) -> usize {
    const OFFSETS: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let adjusted_year = year - i32::from(month < 3);
    ((adjusted_year + adjusted_year / 4 - adjusted_year / 100
        + adjusted_year / 400
        + OFFSETS[month as usize - 1]
        + day as i32)
        .rem_euclid(7)) as usize
}

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// A controlled date/time field whose calendar and time controls appear in an
/// absolute portal popup. The popup paints over adjacent content and escapes
/// scroll-view clipping without changing layout flow.
pub struct DateTimePicker {
    theme: Theme,
    controller: crate::DateTimeController,
    style: Style,
    show_date: bool,
    show_time: bool,
    minute_step: u8,
    popup_width: f32,
    disabled: bool,
}

impl DateTimePicker {
    pub fn default_style() -> Style {
        Style {
            size: fixed(240., 38.),
            flex_shrink: 0.,
            ..Default::default()
        }
    }
    pub fn controlled(controller: &crate::DateTimeController) -> Self {
        Self::controlled_with_style(Self::default_style(), controller)
    }
    pub fn controlled_with_style(style: Style, controller: &crate::DateTimeController) -> Self {
        let theme = use_theme();
        Self {
            theme,
            controller: controller.clone(),
            style,
            show_date: true,
            show_time: true,
            minute_step: 5,
            popup_width: 316.,
            disabled: false,
        }
    }
    pub fn date_only(mut self) -> Self {
        self.show_time = false;
        self
    }
    pub fn time_only(mut self) -> Self {
        self.show_date = false;
        self
    }
    pub fn show_date(mut self, show: bool) -> Self {
        self.show_date = show;
        self
    }
    pub fn show_time(mut self, show: bool) -> Self {
        self.show_time = show;
        self
    }
    pub fn minute_step(mut self, step: u8) -> Self {
        self.minute_step = step.clamp(1, 59);
        self
    }
    pub fn popup_width(mut self, width: f32) -> Self {
        self.popup_width = width.max(220.);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    fn label(&self) -> String {
        let value = self.controller.value();
        match (self.show_date, self.show_time) {
            (true, true) => value.display(),
            (true, false) => value.display()[..10].to_owned(),
            (false, true) => value.display()[13..].to_owned(),
            (false, false) => "Choose a value…".to_owned(),
        }
    }
    fn calendar(&self, value: DateTime) -> BoxedWidget {
        let previous = self.controller.clone();
        let next = self.controller.clone();
        let header = RawView::new(Style {
            align_items: Some(AlignItems::Center),
            justify_content: Some(JustifyContent::SpaceBetween),
            ..row(self.theme.spacing_small)
        })
        .child(Box::new(popup_button("‹", 32., move || {
            previous.set(month_shift(previous.peek(), -1))
        })))
        .child(Box::new(
            RawText::new(
                format!("{} {}", MONTH_NAMES[value.month as usize - 1], value.year),
                self.theme.text_primary,
                13.,
            )
            .bold(true),
        ))
        .child(Box::new(popup_button("›", 32., move || {
            next.set(month_shift(next.peek(), 1))
        })));
        let mut grid = RawView::new(column(3.));
        let mut weekdays = RawView::new(row(3.));
        for name in ["S", "M", "T", "W", "T", "F", "S"] {
            weekdays = weekdays.child(Box::new(
                RawText::new(name, self.theme.text_disabled, 10.).layout_style(Style {
                    size: fixed(36., 18.),
                    ..Default::default()
                }),
            ));
        }
        grid = grid.child(Box::new(weekdays));
        let first = weekday(value.year, value.month, 1);
        let days = DateTime::days_in_month(value.year, value.month) as usize;
        for week in 0..6 {
            let mut row_view = RawView::new(row(3.));
            for weekday in 0..7 {
                let index = week * 7 + weekday;
                if index < first || index >= first + days {
                    row_view = row_view.child(Box::new(RawView::new(Style {
                        size: fixed(36., 30.),
                        ..Default::default()
                    })));
                } else {
                    let day = (index - first + 1) as u8;
                    let set = self.controller.clone();
                    let selected = day == value.day;
                    let mut button = RawButton::new(
                        Style {
                            size: fixed(36., 30.),
                            align_items: Some(AlignItems::Center),
                            justify_content: Some(JustifyContent::Center),
                            ..Default::default()
                        },
                        move || {
                            let current = set.peek();
                            set.set(DateTime::new(
                                current.year,
                                current.month,
                                day,
                                current.hour,
                                current.minute,
                            ));
                        },
                    )
                    .background(if selected {
                        self.theme.accent
                    } else {
                        self.theme.surface_elevated
                    })
                    .corner_radius(self.theme.menu_item_radius)
                    .child(Box::new(RawText::new(
                        day.to_string(),
                        if selected {
                            self.theme.selection_text
                        } else {
                            self.theme.text_primary
                        },
                        12.,
                    )));
                    button = button.hover_background(if selected {
                        self.theme.accent_hover
                    } else {
                        self.theme.surface_hover
                    });
                    row_view = row_view.child(Box::new(button));
                }
            }
            grid = grid.child(Box::new(row_view));
        }
        Box::new(
            RawView::new(column(self.theme.spacing_small))
                .child(Box::new(header))
                .child(Box::new(grid)),
        )
    }
    fn time_controls(&self, value: DateTime) -> BoxedWidget {
        let hour_down = self.controller.clone();
        let hour_up = self.controller.clone();
        let minute_down = self.controller.clone();
        let minute_up = self.controller.clone();
        let step = self.minute_step as i32;
        let controls = RawView::new(Style {
            align_items: Some(AlignItems::Center),
            ..row(self.theme.spacing_small)
        })
        .child(Box::new(popup_button("−", 32., move || {
            hour_down.set(hour_down.peek().add_minutes(-60))
        })))
        .child(Box::new(
            RawText::new(format!("{:02}", value.hour), self.theme.text_primary, 18.).layout_style(
                Style {
                    size: fixed(34., 28.),
                    ..Default::default()
                },
            ),
        ))
        .child(Box::new(popup_button("+", 32., move || {
            hour_up.set(hour_up.peek().add_minutes(60))
        })))
        .child(Box::new(RawText::new(":", self.theme.text_secondary, 18.)))
        .child(Box::new(popup_button("−", 32., move || {
            minute_down.set(minute_down.peek().add_minutes(-step))
        })))
        .child(Box::new(
            RawText::new(format!("{:02}", value.minute), self.theme.text_primary, 18.)
                .layout_style(Style {
                    size: fixed(34., 28.),
                    ..Default::default()
                }),
        ))
        .child(Box::new(popup_button("+", 32., move || {
            minute_up.set(minute_up.peek().add_minutes(step))
        })));
        Box::new(
            RawView::new(column(self.theme.spacing_small))
                .child(Box::new(
                    RawText::new("Time", self.theme.text_secondary, 11.).bold(true),
                ))
                .child(Box::new(controls)),
        )
    }
}

impl Widget for DateTimePicker {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.fill_rect(
            rect,
            if !self.disabled && painter.hovered(rect) {
                self.theme.surface_hover
            } else {
                self.theme.surface_elevated
            },
            self.theme.input_radius,
        );
        painter.stroke_rect(
            rect,
            if self.controller.is_open() {
                self.theme.accent
            } else {
                self.theme.border_strong
            },
            self.theme.input_border_width,
            self.theme.input_radius,
        );
        painter.fill_text(
            Rect {
                x: rect.x + 11.,
                y: rect.y,
                width: (rect.width - 34.).max(0.),
                height: rect.height,
            },
            &self.label(),
            self.theme.text_primary,
            self.theme.typography.body,
            TextAlign::Start,
        );
        let cx = rect.x + rect.width - 17.;
        let cy = rect.y + rect.height / 2.;
        let (a, b, c) = if self.controller.is_open() {
            (
                Point {
                    x: cx - 4.,
                    y: cy + 2.,
                },
                Point {
                    x: cx + 4.,
                    y: cy + 2.,
                },
                Point { x: cx, y: cy - 2. },
            )
        } else {
            (
                Point {
                    x: cx - 4.,
                    y: cy - 2.,
                },
                Point {
                    x: cx + 4.,
                    y: cy - 2.,
                },
                Point { x: cx, y: cy + 2. },
            )
        };
        painter.stroke_line(a, b, self.theme.text_secondary, 1.5);
        painter.stroke_line(b, c, self.theme.text_secondary, 1.5);
        painter.stroke_line(c, a, self.theme.text_secondary, 1.5);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        if !self.controller.is_open() || self.disabled {
            return Vec::new();
        }
        let theme = self.theme;
        {
            let value = self.controller.value();
            let mut content = RawView::new(column(theme.spacing_medium));
            if self.show_date {
                content = content.child(self.calendar(value));
            }
            if self.show_time {
                content = content.child(self.time_controls(value));
            }
            let close = self.controller.clone();
            let done = RawButton::new(
                Style {
                    size: fixed(72., 28.),
                    align_self: Some(creamui_core::layout::AlignSelf::End),
                    align_items: Some(AlignItems::Center),
                    justify_content: Some(JustifyContent::Center),
                    ..Default::default()
                },
                move || close.set_open(false),
            )
            .background(theme.accent)
            .corner_radius(theme.menu_item_radius)
            .child(Box::new(
                RawText::new("Done", theme.selection_text, 12.).align(TextAlign::Center),
            ));
            content = content.child(Box::new(done));
            let dismiss = self.controller.clone();
            vec![
                super::portal_dismiss_layer(move || dismiss.set_open(false)),
                Box::new(
                    Popover::new(popup_style(
                        self.popup_width,
                        44.,
                        theme.spacing_medium,
                        theme.spacing_medium,
                    ))
                    .child(Box::new(content)),
                ) as BoxedWidget,
            ]
        }
    }
    fn focusable(&self) -> bool {
        !self.disabled
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        (!self.disabled).then(|| {
            let controller = self.controller.clone();
            Rc::new(move || controller.toggle()) as Rc<dyn Fn()>
        })
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        (!self.disabled).then(|| {
            let controller = self.controller.clone();
            Rc::new(move |input: KeyInput| match input.key {
                Key::Enter | Key::Char(' ') => controller.toggle(),
                Key::Escape => controller.set_open(false),
                _ => {}
            }) as Rc<dyn Fn(KeyInput)>
        })
    }
    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, _: bool) {
        painter.stroke_rect(rect, self.theme.accent, 2., self.theme.input_radius);
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }
}

/// Date-only configuration of [`DateTimePicker`].
pub struct DateInput {
    inner: DateTimePicker,
}
impl DateInput {
    pub fn controlled(controller: &crate::DateTimeController) -> Self {
        Self {
            inner: DateTimePicker::controlled(controller).date_only(),
        }
    }
    pub fn controlled_with_style(style: Style, controller: &crate::DateTimeController) -> Self {
        Self {
            inner: DateTimePicker::controlled_with_style(style, controller).date_only(),
        }
    }
    pub fn minute_step(mut self, step: u8) -> Self {
        self.inner = self.inner.minute_step(step);
        self
    }
    pub fn popup_width(mut self, width: f32) -> Self {
        self.inner = self.inner.popup_width(width);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.inner = self.inner.disabled(disabled);
        self
    }
}
impl Widget for DateInput {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, p: &mut dyn Painter, r: Rect) {
        self.inner.paint(p, r)
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
    fn focusable(&self) -> bool {
        self.inner.focusable()
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }
    fn paint_focused_overlay(&self, p: &mut dyn Painter, r: Rect, c: bool) {
        self.inner.paint_focused_overlay(p, r, c)
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }
}

/// Time-only configuration of [`DateTimePicker`].
pub struct TimeInput {
    inner: DateTimePicker,
}
impl TimeInput {
    pub fn controlled(controller: &crate::DateTimeController) -> Self {
        Self {
            inner: DateTimePicker::controlled(controller).time_only(),
        }
    }
    pub fn controlled_with_style(style: Style, controller: &crate::DateTimeController) -> Self {
        Self {
            inner: DateTimePicker::controlled_with_style(style, controller).time_only(),
        }
    }
    pub fn minute_step(mut self, step: u8) -> Self {
        self.inner = self.inner.minute_step(step);
        self
    }
    pub fn popup_width(mut self, width: f32) -> Self {
        self.inner = self.inner.popup_width(width);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.inner = self.inner.disabled(disabled);
        self
    }
}
impl Widget for TimeInput {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, p: &mut dyn Painter, r: Rect) {
        self.inner.paint(p, r)
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
    fn focusable(&self) -> bool {
        self.inner.focusable()
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }
    fn paint_focused_overlay(&self, p: &mut dyn Painter, r: Rect, c: bool) {
        self.inner.paint_focused_overlay(p, r, c)
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }
}

/// A controlled color field with an HSV palette in a portal popup. Keep the
/// [`crate::ColorPickerController`] in application state to retain its popup.
pub struct ColorPicker {
    theme: Theme,
    value: Color,
    controller: crate::ColorPickerController,
    style: Style,
    popup_width: f32,
    on_change: Rc<dyn Fn(Color)>,
    disabled: bool,
}
impl ColorPicker {
    pub fn default_style() -> Style {
        Style {
            size: fixed(220., 38.),
            flex_shrink: 0.,
            ..Default::default()
        }
    }
    pub fn controlled(
        value: Color,
        controller: &crate::ColorPickerController,
        on_change: impl Fn(Color) + 'static,
    ) -> Self {
        Self::controlled_with_style(Self::default_style(), value, controller, on_change)
    }
    pub fn controlled_with_style(
        style: Style,
        value: Color,
        controller: &crate::ColorPickerController,
        on_change: impl Fn(Color) + 'static,
    ) -> Self {
        let theme = use_theme();
        Self {
            theme,
            value,
            controller: controller.clone(),
            style,
            popup_width: 292.,
            on_change: Rc::new(on_change),
            disabled: false,
        }
    }
    pub fn popup_width(mut self, width: f32) -> Self {
        self.popup_width = width.max(220.);
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}
impl Widget for ColorPicker {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.fill_rect(
            rect,
            if !self.disabled && painter.hovered(rect) {
                self.theme.surface_hover
            } else {
                self.theme.surface_elevated
            },
            self.theme.input_radius,
        );
        painter.stroke_rect(
            rect,
            if self.controller.is_open() {
                self.theme.accent
            } else {
                self.theme.border_strong
            },
            self.theme.input_border_width,
            self.theme.input_radius,
        );
        painter.fill_rect(
            Rect {
                x: rect.x + 7.,
                y: rect.y + 7.,
                width: (rect.height - 14.).max(0.),
                height: (rect.height - 14.).max(0.),
            },
            self.value,
            (self.theme.input_radius - 2.).max(0.),
        );
        painter.fill_text(
            Rect {
                x: rect.x + rect.height + 3.,
                y: rect.y,
                width: (rect.width - rect.height - 31.).max(0.),
                height: rect.height,
            },
            &format!(
                "#{:02X}{:02X}{:02X}",
                self.value.r, self.value.g, self.value.b
            ),
            self.theme.text_primary,
            self.theme.typography.body,
            TextAlign::Start,
        );
        painter.fill_text(
            Rect {
                x: rect.x + rect.width - 23.,
                y: rect.y,
                width: 16.,
                height: rect.height,
            },
            if self.controller.is_open() {
                "⌃"
            } else {
                "⌄"
            },
            self.theme.text_secondary,
            15.,
            TextAlign::Center,
        );
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        if !self.controller.is_open() || self.disabled {
            return Vec::new();
        }
        let theme = self.theme;
        let popup_width = self.popup_width;
        let change = self.on_change.clone();
        let close = self.controller.clone();
        let value = self.value;
        {
            let picker = RawColorPicker::new(
                Style {
                    size: fixed(popup_width - theme.spacing_medium * 2., 188.),
                    ..Default::default()
                },
                value,
                theme.border_strong,
                theme.accent,
                move |next| change(next),
            )
            .background(theme.surface_elevated)
            .border(theme.border_strong, theme.input_border_width)
            .corner_radius(theme.input_radius)
            .focus_color(theme.accent);
            let summary = RawView::new(Style {
                align_items: Some(AlignItems::Center),
                justify_content: Some(JustifyContent::SpaceBetween),
                ..row(theme.spacing_small)
            })
            .child(Box::new(
                RawView::new(Style {
                    size: fixed(28., 28.),
                    ..Default::default()
                })
                .background(value)
                .corner_radius(8.),
            ))
            .child(Box::new(
                RawText::new(
                    format!("#{:02X}{:02X}{:02X}", value.r, value.g, value.b),
                    theme.text_primary,
                    13.,
                )
                .layout_style(Style {
                    flex_grow: 1.,
                    ..Default::default()
                }),
            ))
            .child(Box::new(popup_button("Done", 64., move || {
                close.set_open(false)
            })));
            let dismiss = self.controller.clone();
            vec![
                super::portal_dismiss_layer(move || dismiss.set_open(false)),
                Box::new(
                    Popover::new(popup_style(
                        popup_width,
                        44.,
                        theme.spacing_medium,
                        theme.spacing_medium,
                    ))
                    .child(Box::new(
                        RawView::new(column(theme.spacing_medium))
                            .child(Box::new(picker))
                            .child(Box::new(summary)),
                    )),
                ) as BoxedWidget,
            ]
        }
    }
    fn focusable(&self) -> bool {
        !self.disabled
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        (!self.disabled).then(|| {
            let controller = self.controller.clone();
            Rc::new(move || controller.toggle()) as Rc<dyn Fn()>
        })
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        (!self.disabled).then(|| {
            let controller = self.controller.clone();
            Rc::new(move |input: KeyInput| match input.key {
                Key::Enter | Key::Char(' ') => controller.toggle(),
                Key::Escape => controller.set_open(false),
                _ => {}
            }) as Rc<dyn Fn(KeyInput)>
        })
    }
    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, _: bool) {
        painter.stroke_rect(rect, self.theme.accent, 2., self.theme.input_radius);
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }
}

/// A themed file picker that delegates choosing the file to the platform's native dialog. Use [`RawFilePicker`] when the file source is not local.
pub struct FilePicker {
    theme: Theme,
    style: Style,
    value: String,
    placeholder: String,
    title: String,
    filters: Vec<(String, Vec<String>)>,
    on_change: Rc<dyn Fn(PathBuf)>,
    disabled: bool,
}
impl FilePicker {
    pub fn default_style() -> Style {
        Style {
            size: fixed(320., 40.),
            flex_shrink: 0.,
            ..Default::default()
        }
    }
    pub fn new(value: impl Into<String>, on_change: impl Fn(PathBuf) + 'static) -> Self {
        Self::with_style(Self::default_style(), value, on_change)
    }
    pub fn with_style(
        style: Style,
        value: impl Into<String>,
        on_change: impl Fn(PathBuf) + 'static,
    ) -> Self {
        let theme = use_theme();
        Self {
            theme,
            style,
            value: value.into(),
            placeholder: "Choose a file…".into(),
            title: "Choose a file".into(),
            filters: Vec::new(),
            on_change: Rc::new(on_change),
            disabled: false,
        }
    }
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }
    pub fn title(mut self, text: impl Into<String>) -> Self {
        self.title = text.into();
        self
    }
    pub fn filter(
        mut self,
        name: impl Into<String>,
        extensions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.filters.push((
            name.into(),
            extensions.into_iter().map(Into::into).collect(),
        ));
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    fn raw(&self) -> RawFilePicker {
        let callback = self.on_change.clone();
        let title = self.title.clone();
        let filters = self.filters.clone();
        RawFilePicker::new(
            self.style.clone(),
            self.value.clone(),
            self.theme.text_primary,
            self.theme.text_disabled,
            self.theme.border,
            move || {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let mut dialog = rfd::FileDialog::new().set_title(&title);
                    for (name, extensions) in &filters {
                        dialog = dialog.add_filter(name, extensions);
                    }
                    if let Some(path) = dialog.pick_file() {
                        callback(path);
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    // Browsers cannot expose a native path synchronously.
                    // The control remains visible so the showcase documents
                    // it, but selecting local files needs an async web host.
                    let _ = (&callback, &title, &filters);
                }
            },
        )
        .placeholder(self.placeholder.clone())
        .background(self.theme.surface_elevated)
        .hover_background(self.theme.surface_hover)
        .border(self.theme.border_strong, self.theme.input_border_width)
        .corner_radius(self.theme.input_radius)
        .focus_color(self.theme.accent)
        .disabled(self.disabled)
    }
}
impl Widget for FilePicker {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }
    fn paint(&self, p: &mut dyn Painter, r: Rect) {
        self.raw().paint(p, r)
    }
    fn focusable(&self) -> bool {
        !self.disabled
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.raw().on_key()
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.raw().on_click()
    }
    fn paint_focused_overlay(&self, p: &mut dyn Painter, r: Rect, c: bool) {
        self.raw().paint_focused_overlay(p, r, c)
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.raw().cursor_icon()
    }
}
