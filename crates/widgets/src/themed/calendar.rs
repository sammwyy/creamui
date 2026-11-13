use super::*;
use crate::layout::StyleExt;
use crate::raw::{CalendarColors, CalendarNavigation, DateTime, RawCalendar};

pub struct Calendar {
    inner: RawCalendar,
}
impl_styled_inner!(Calendar);

impl Calendar {
    pub fn new(
        month: DateTime,
        selected: Option<DateTime>,
        on_select: impl Fn(DateTime) + 'static,
        on_month: impl Fn(DateTime) + 'static,
    ) -> Self {
        let theme = use_theme();
        Self {
            inner: RawCalendar::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Auto,
                    },
                    ..Style::default()
                        .flex_column()
                        .padding_all(theme.spacing_medium)
                },
                month,
                selected,
                on_select,
                on_month,
            )
            .colors(CalendarColors {
                text: theme.text_primary,
                muted: theme.text_secondary,
                selected_text: theme.accent.readable_text(),
                selected: theme.accent,
                today: theme.accent,
            })
            .navigation(CalendarNavigation {
                previous: Box::new(
                    crate::Icon::new(crate::Symbol::ChevronLeft, theme.text_secondary).size(16.0),
                ),
                next: Box::new(
                    crate::Icon::new(crate::Symbol::ChevronRight, theme.text_secondary).size(16.0),
                ),
                button: creamui_core::Style::from(
                    Style::default()
                        .flex_row()
                        .align(crate::layout::Align::Center)
                        .justify(crate::layout::Justify::Center)
                        .size(28.0, 28.0),
                )
                .corner_radius(theme.button_radius)
                .hover(creamui_core::StateStyle::new().background(elevation::subtle(&theme, 1)))
                .pressed(creamui_core::StateStyle::new().background(elevation::subtle(&theme, 2))),
            })
            .day_hover(elevation::subtle(&theme, 1))
            .background(elevation::group_fill(&theme))
            .border(theme.border, 1.0)
            .corner_radius(theme.card_radius),
        }
    }

    pub fn week_start(mut self, day: usize) -> Self {
        self.inner = self.inner.week_start(day);
        self
    }

    pub fn today(mut self, date: DateTime) -> Self {
        self.inner = self.inner.today(date);
        self
    }

    pub fn disabled(mut self, predicate: impl Fn(DateTime) -> bool + 'static) -> Self {
        self.inner = self.inner.disabled(predicate);
        self
    }
}

impl Widget for Calendar {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themed_calendar_keeps_month_grid_and_theme_color() {
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::light();
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme.clone()));
            let mut calendar =
                Calendar::new(DateTime::new(2026, 3, 1, 0, 0), None, |_| {}, |_| {}).week_start(1);
            assert_eq!(calendar.children().len(), 8);
            assert_eq!(
                calendar.style().paint.background,
                Some(elevation::group_fill(&theme).into())
            );
        });
    }
}
