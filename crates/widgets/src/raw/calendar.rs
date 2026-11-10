use super::*;
use creamui_core::layout::{AlignItems, Dimension, FlexDirection, JustifyContent};

const MONTHS: [&str; 12] = [
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
const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

fn weekday(year: i32, month: u8, day: u8) -> usize {
    let offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let year = if month < 3 { year - 1 } else { year };
    (year + year / 4 - year / 100 + year / 400 + offsets[month as usize - 1] + day as i32)
        .rem_euclid(7) as usize
}

fn previous_month(month: DateTime) -> DateTime {
    let (year, month) = if month.month == 1 {
        (month.year.saturating_sub(1), 12)
    } else {
        (month.year, month.month - 1)
    };
    DateTime::new(year, month, 1, 0, 0)
}

fn next_month(month: DateTime) -> DateTime {
    let (year, month) = if month.month == 12 {
        (month.year.saturating_add(1), 1)
    } else {
        (month.year, month.month + 1)
    };
    DateTime::new(year, month, 1, 0, 0)
}

#[derive(Clone, Copy)]
pub struct CalendarColors {
    pub text: Color,
    pub muted: Color,
    pub selected_text: Color,
    pub selected: Color,
    pub today: Color,
}

pub struct RawCalendar {
    pub style: creamui_core::Style,
    month: DateTime,
    selected: Option<DateTime>,
    today: Option<DateTime>,
    week_start: usize,
    disabled: Option<Rc<dyn Fn(DateTime) -> bool>>,
    on_select: Rc<dyn Fn(DateTime)>,
    on_month: Rc<dyn Fn(DateTime)>,
    colors: CalendarColors,
}

impl RawCalendar {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        month: DateTime,
        selected: Option<DateTime>,
        on_select: impl Fn(DateTime) + 'static,
        on_month: impl Fn(DateTime) + 'static,
    ) -> Self {
        Self {
            style: style.into(),
            month: DateTime::new(month.year, month.month, 1, 0, 0),
            selected,
            today: None,
            week_start: 0,
            disabled: None,
            on_select: Rc::new(on_select),
            on_month: Rc::new(on_month),
            colors: CalendarColors {
                text: Color::rgb(0, 0, 0),
                muted: Color::rgb(100, 100, 100),
                selected_text: Color::rgb(255, 255, 255),
                selected: Color::rgb(35, 90, 180),
                today: Color::rgb(35, 90, 180),
            },
        }
    }

    pub fn week_start(mut self, day: usize) -> Self {
        self.week_start = day % 7;
        self
    }

    pub fn today(mut self, day: DateTime) -> Self {
        self.today = Some(day);
        self
    }

    pub fn disabled(mut self, predicate: impl Fn(DateTime) -> bool + 'static) -> Self {
        self.disabled = Some(Rc::new(predicate));
        self
    }

    pub fn colors(mut self, colors: CalendarColors) -> Self {
        self.colors = colors;
        self
    }

    fn grid_start(&self) -> DateTime {
        let leading = (weekday(self.month.year, self.month.month, 1) + 7 - self.week_start) % 7;
        self.month.add_days(-(leading as i32))
    }
}

impl Widget for RawCalendar {
    fn style(&self) -> creamui_core::Style {
        let mut style = self.style.clone();
        style.layout.flex_direction = FlexDirection::Column;
        style
    }

    fn paint(&self, _: &mut dyn Painter, _: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        let header_style = Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(36.0),
            },
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            justify_content: Some(JustifyContent::SpaceBetween),
            ..Default::default()
        };
        let previous = previous_month(self.month);
        let next = next_month(self.month);
        let on_previous = self.on_month.clone();
        let on_next = self.on_month.clone();
        let header = RawView::new(header_style)
            .child(Box::new(
                RawButton::new(Style::default(), move || on_previous(previous))
                    .child(Box::new(RawText::new("<", self.colors.text, 14.0))),
            ))
            .child(Box::new(RawText::new(
                format!(
                    "{} {}",
                    MONTHS[self.month.month as usize - 1],
                    self.month.year
                ),
                self.colors.text,
                14.0,
            )))
            .child(Box::new(
                RawButton::new(Style::default(), move || on_next(next))
                    .child(Box::new(RawText::new(">", self.colors.text, 14.0))),
            ));
        let mut children = vec![Box::new(header) as BoxedWidget];
        let column_width = 36.0;
        for row in 0..7 {
            let mut week = RawView::new(Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(36.0),
                },
                flex_direction: FlexDirection::Row,
                ..Default::default()
            });
            for column in 0..7 {
                let cell_style = Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0 / 7.0),
                        height: Dimension::Length(36.0),
                    },
                    align_items: Some(AlignItems::Center),
                    justify_content: Some(JustifyContent::Center),
                    ..Default::default()
                };
                if row == 0 {
                    let weekday = DAYS[(column + self.week_start) % 7];
                    week = week.child(Box::new(
                        RawText::new(weekday, self.colors.muted, 11.0)
                            .layout(cell_style)
                            .text_align(TextAlign::Center),
                    ));
                    continue;
                }
                let day_style = Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(32.0),
                        height: Dimension::Length(32.0),
                    },
                    align_items: Some(AlignItems::Center),
                    justify_content: Some(JustifyContent::Center),
                    ..Default::default()
                };
                let date = self.grid_start().add_days(((row - 1) * 7 + column) as i32);
                let disabled = self
                    .disabled
                    .as_ref()
                    .is_some_and(|predicate| predicate(date));
                let selected = self.selected.is_some_and(|day| {
                    (day.year, day.month, day.day) == (date.year, date.month, date.day)
                });
                let today = self.today.is_some_and(|day| {
                    (day.year, day.month, day.day) == (date.year, date.month, date.day)
                });
                let text_color = if selected {
                    self.colors.selected_text
                } else if disabled || date.month != self.month.month {
                    self.colors.muted
                } else {
                    self.colors.text
                };
                let on_select = self.on_select.clone();
                let mut button = RawButton::new(day_style, move || on_select(date))
                    .disabled(disabled)
                    .child(Box::new(RawText::new(
                        date.day.to_string(),
                        text_color,
                        13.0,
                    )));
                if selected {
                    button = button
                        .background(self.colors.selected)
                        .corner_radius(column_width / 2.0);
                } else if today {
                    button = button
                        .border(self.colors.today, 1.5)
                        .corner_radius(column_width / 2.0);
                }
                let on_select_key = self.on_select.clone();
                let disabled_predicate = self.disabled.clone();
                button = button.on_key_press(move |input| {
                    let offset = match input.key {
                        Key::Left => -1,
                        Key::Right => 1,
                        Key::Up => -7,
                        Key::Down => 7,
                        _ => return,
                    };
                    let target = date.add_days(offset);
                    if !disabled_predicate
                        .as_ref()
                        .is_some_and(|predicate| predicate(target))
                    {
                        on_select_key(target);
                    }
                });
                week = week.child(Box::new(RawView::new(cell_style).child(Box::new(button))));
            }
            children.push(Box::new(week));
        }
        children
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekdays_and_week_start_handle_leap_and_year_boundaries() {
        assert_eq!(weekday(2026, 3, 1), 0);
        assert_eq!(weekday(2024, 2, 29), 4);
        let month = DateTime::new(2026, 3, 1, 0, 0);
        let sunday = RawCalendar::new(Style::default(), month, None, |_| {}, |_| {});
        assert_eq!(sunday.grid_start(), month);
        assert_eq!(
            sunday.week_start(1).grid_start(),
            DateTime::new(2026, 2, 23, 0, 0)
        );
        assert_eq!(previous_month(DateTime::new(2026, 1, 1, 0, 0)).year, 2025);
    }

    #[test]
    fn calendar_selection_and_disabled_days() {
        let selected = Rc::new(Cell::new(None));
        let set_selected = selected.clone();
        let mut calendar = RawCalendar::new(
            Style::default(),
            DateTime::new(2026, 3, 1, 0, 0),
            None,
            move |date| set_selected.set(Some(date)),
            |_| {},
        )
        .disabled(|date| date.day == 1);
        let mut children = calendar.children();
        assert_eq!(children.len(), 8);
        let mut first_week = children[2].children();
        assert!(first_week[0].children()[0].on_click().is_none());
        first_week[1].children()[0].on_click().unwrap()();
        assert_eq!(selected.get().unwrap().day, 2);
    }
}
