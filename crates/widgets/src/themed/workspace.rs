use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::{RawDescriptionList, RawStatusBar, RawStepper, StepState};

pub struct DescriptionList {
    inner: RawDescriptionList,
}
impl_styled_inner!(DescriptionList);

impl DescriptionList {
    pub fn new() -> Self {
        let theme = use_theme();
        Self {
            inner: RawDescriptionList::new(Style::default().flex_column().gap(theme.spacing_large)),
        }
    }

    pub fn horizontal(mut self, term_width: f32) -> Self {
        self.inner = self.inner.term_width(term_width);
        self
    }

    pub fn entry(mut self, term: impl Into<String>, value: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.entry(
            Box::new(RawText::new(
                term,
                theme.text_secondary,
                theme.typography.caption,
            )),
            Box::new(RawText::new(
                value,
                theme.text_primary,
                theme.typography.body,
            )),
        );
        self
    }
}

impl Default for DescriptionList {
    fn default() -> Self {
        Self::new()
    }
}

pub struct StatusBar {
    inner: RawStatusBar,
}
impl_styled_inner!(StatusBar);

impl StatusBar {
    pub fn new() -> Self {
        let theme = use_theme();
        Self {
            inner: RawStatusBar::new(Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(32.0),
                },
                ..Style::default()
                    .flex_row()
                    .align(Align::Center)
                    .padding_xy(theme.spacing_medium, theme.spacing_small)
            })
            .background(elevation::with_alpha(theme.text_primary, 8))
            .border(theme.border, 1.0)
            .corner_radius(theme.radius_small),
        }
    }

    pub fn start(mut self, widget: BoxedWidget) -> Self {
        self.inner = self.inner.start(widget);
        self
    }

    pub fn center(mut self, widget: BoxedWidget) -> Self {
        self.inner = self.inner.center(widget);
        self
    }

    pub fn end(mut self, widget: BoxedWidget) -> Self {
        self.inner = self.inner.end(widget);
        self
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Stepper {
    inner: RawStepper,
    count: usize,
}
impl_styled_inner!(Stepper);

impl Stepper {
    pub fn new(on_select: impl Fn(usize) + 'static) -> Self {
        let theme = use_theme();
        Self {
            inner: RawStepper::new(
                Style::default()
                    .flex_row()
                    .align(Align::Center)
                    .gap(theme.spacing_medium),
                on_select,
            ),
            count: 0,
        }
    }

    pub fn step(mut self, label: impl Into<String>, state: StepState) -> Self {
        let theme = use_theme();
        self.count += 1;
        let active = state == StepState::Active;
        let completed = state == StepState::Completed;
        let foreground = if state == StepState::Disabled {
            theme.text_disabled
        } else {
            theme.text_primary
        };
        let indicator = RawView::new(
            Style::default()
                .flex_row()
                .align(Align::Center)
                .justify(crate::layout::Justify::Center)
                .size(22.0, 22.0),
        )
        .background(if active || completed {
            theme.accent
        } else {
            elevation::rail_fill(&theme)
        })
        .corner_radius(theme.radius_large.min(11.0))
        .child(Box::new(
            RawText::new(
                self.count.to_string(),
                if active || completed {
                    theme.accent.readable_text()
                } else {
                    foreground
                },
                theme.typography.caption,
            )
            .bold(active),
        ));
        let item = RawView::new(
            Style::default()
                .flex_row()
                .align(Align::Center)
                .gap(theme.spacing_medium)
                .padding_xy(
                    theme.spacing_medium + theme.spacing_small,
                    theme.spacing_medium,
                ),
        )
        .background(if active {
            elevation::with_alpha(theme.accent, 36)
        } else {
            elevation::group_fill(&theme)
        })
        .border(
            if active {
                elevation::with_alpha(theme.accent, 150)
            } else {
                theme.border
            },
            1.0,
        )
        .corner_radius(theme.radius_medium)
        .child(Box::new(indicator))
        .child(Box::new(RawText::new(
            label,
            foreground,
            theme.typography.body,
        )));
        self.inner = self.inner.step(Box::new(item), state);
        self
    }

    pub fn vertical(mut self) -> Self {
        self.inner.style.layout.flex_direction = creamui_core::layout::FlexDirection::Column;
        self
    }
}

macro_rules! impl_workspace_widget {
    ($($widget:ident),+ $(,)?) => {
        $(
            impl Widget for $widget {
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
        )+
    };
}

impl_workspace_widget!(DescriptionList, StatusBar, Stepper);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themed_workspace_widgets_mount() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let slot = || Box::new(RawView::new(Style::default())) as BoxedWidget;
            let mut list = DescriptionList::new().entry("State", "Ready");
            assert_eq!(list.children().len(), 1);
            let mut bar = StatusBar::new().start(slot());
            assert_eq!(bar.children().len(), 3);
            let mut steps = Stepper::new(|_| {})
                .step("Create", StepState::Completed)
                .step("Review", StepState::Active);
            assert_eq!(steps.children().len(), 2);
        });
    }

    #[test]
    fn steps_keep_state_distinct_from_label_and_respect_theme_corners() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let theme = use_theme();
            let mut steps = Stepper::new(|_| {})
                .step("Create", StepState::Completed)
                .step("Review", StepState::Active);
            let mut items = steps.children();
            let mut active_button = items[1].children();
            assert_eq!(
                active_button[0].style().paint.corner_radius,
                Some(theme.radius_medium)
            );
            let parts = active_button[0].children();
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0].style().layout.size.width, Dimension::Length(22.0));
            assert_eq!(
                parts[0].style().paint.corner_radius,
                Some(theme.radius_large.min(11.0))
            );
        });
    }
}
