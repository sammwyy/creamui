use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::{PaginationItemStyles, RawAccordion, RawField, RawForm, RawPagination};

pub struct Accordion {
    inner: RawAccordion,
}
impl_styled_inner!(Accordion);

impl Accordion {
    pub fn new(on_toggle: impl Fn(usize, bool) + 'static) -> Self {
        let theme = use_theme();
        Self {
            inner: RawAccordion::new(
                Style::default().flex_column().gap(theme.spacing_small),
                on_toggle,
            ),
        }
    }

    pub fn item(
        mut self,
        heading: impl Into<String>,
        content: BoxedWidget,
        expanded: bool,
    ) -> Self {
        let theme = use_theme();
        let header = RawView::new(Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            ..Style::default()
                .flex_row()
                .gap(theme.spacing_medium)
                .align(Align::Center)
                .padding_xy(theme.spacing_large, theme.spacing_medium + 2.0)
        })
        .background(elevation::group_fill(&theme))
        .border(theme.border, 1.0)
        .corner_radius(theme.radius_medium)
        .hover_style(
            creamui_core::StateStyle::new()
                .background(elevation::group_fill(&theme).mix(theme.text_primary, 0.04)),
        )
        .child(Box::new(
            crate::Icon::new(
                if expanded {
                    crate::Symbol::ChevronDown
                } else {
                    crate::Symbol::ChevronRight
                },
                theme.text_secondary,
            )
            .size(14.0),
        ))
        .child(Box::new(
            RawText::new(heading, theme.text_primary, theme.typography.body)
                .bold(expanded)
                .font_family(theme.font_family),
        ));
        let content = RawView::new(
            Style::default()
                .flex_column()
                .padding_xy(theme.spacing_large, theme.spacing_medium),
        )
        .child(content);
        self.inner = self
            .inner
            .item(Box::new(header), Box::new(content), expanded);
        self
    }
}

pub struct Field {
    inner: RawField,
}
impl_styled_inner!(Field);

impl Field {
    pub fn new(label: impl Into<String>, control: BoxedWidget, required: bool) -> Self {
        let theme = use_theme();
        let label = format!("{}{}", label.into(), if required { " *" } else { "" });
        Self {
            inner: RawField::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Auto,
                    },
                    min_size: creamui_core::layout::Size {
                        width: Dimension::Length(0.0),
                        height: Dimension::Auto,
                    },
                    ..Style::default().flex_column().gap(theme.spacing_small)
                },
                Box::new(RawText::new(
                    label,
                    theme.text_primary,
                    theme.typography.body,
                )),
                control,
            )
            .required(required),
        }
    }

    pub fn hint(mut self, text: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.hint(Box::new(RawText::new(
            text,
            theme.text_secondary,
            theme.typography.caption,
        )));
        self
    }

    pub fn error(mut self, text: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.error(Box::new(RawText::new(
            text,
            theme.danger,
            theme.typography.caption,
        )));
        self
    }
}

pub struct Form {
    inner: RawForm,
}
impl_styled_inner!(Form);

impl Form {
    pub fn new() -> Self {
        let theme = use_theme();
        Self {
            inner: RawForm::new(Style::default().flex_column().gap(theme.spacing_medium)),
        }
    }

    pub fn field(mut self, field: BoxedWidget) -> Self {
        self.inner = self.inner.field(field);
        self
    }

    pub fn error(mut self, error: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.summary(Box::new(RawText::new(
            error,
            theme.danger,
            theme.typography.body,
        )));
        self
    }

    pub fn submit(mut self, label: impl Into<String>, on_submit: impl Fn() + 'static) -> Self {
        let theme = use_theme();
        self.inner = self
            .inner
            .submit(
                Box::new(
                    RawText::new(label, theme.accent.readable_text(), theme.typography.body)
                        .bold(true),
                ),
                on_submit,
            )
            .submit_style(
                creamui_core::Style::from(Style {
                    align_self: Some(creamui_core::layout::AlignSelf::Start),
                    ..Style::default()
                        .flex_row()
                        .align(Align::Center)
                        .justify(crate::layout::Justify::Center)
                        .padding_xy(14.0, 0.0)
                })
                .height(34.0)
                .background(theme.accent)
                .corner_radius(theme.button_radius)
                .hover(creamui_core::StateStyle::new().background(theme.accent_hover))
                .pressed(creamui_core::StateStyle::new().background(theme.accent_pressed))
                .disabled(creamui_core::StateStyle::new().background(elevation::subtle(&theme, 1)))
                .focus(
                    creamui_core::StateStyle::new()
                        .outline(elevation::focus_ring(&theme), elevation::FOCUS_RING_WIDTH),
                ),
            );
        self
    }

    pub fn submitting(mut self, submitting: bool) -> Self {
        self.inner = self.inner.submitting(submitting);
        self
    }
}

impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Pagination {
    inner: RawPagination,
}
impl_styled_inner!(Pagination);

impl Pagination {
    pub fn new(current: usize, total: usize, on_change: impl Fn(usize) + 'static) -> Self {
        let theme = use_theme();
        Self {
            inner: RawPagination::new(
                Style::default()
                    .flex_row()
                    .align(Align::Center)
                    .gap(theme.spacing_small),
                current,
                total,
                on_change,
            )
            .item_styles(pagination_styles(&theme)),
        }
    }

    pub fn compact(mut self) -> Self {
        self.inner = self.inner.compact();
        self
    }
}

fn pagination_styles(theme: &Theme) -> PaginationItemStyles {
    let base = || {
        creamui_core::Style::from(
            Style::default()
                .flex_row()
                .align(Align::Center)
                .justify(crate::layout::Justify::Center)
                .padding_xy(8.0, 0.0),
        )
        .min_width(32.0)
        .height(32.0)
        .corner_radius(theme.button_radius)
    };
    let clear = elevation::with_alpha(theme.text_primary, 0);
    let item = base()
        .background(clear)
        .border(clear, 1.0)
        .hover(creamui_core::StateStyle::new().background(elevation::subtle(theme, 1)))
        .pressed(creamui_core::StateStyle::new().background(elevation::subtle(theme, 2)))
        .disabled(
            creamui_core::StateStyle::new()
                .background(clear)
                .color(theme.text_disabled),
        )
        .focus(creamui_core::StateStyle::new().outline(elevation::focus_ring(theme), 2.0));
    let selected = base()
        .background(theme.accent)
        .border(theme.accent, 1.0)
        .focus(creamui_core::StateStyle::new().outline(elevation::focus_ring(theme), 2.0));
    PaginationItemStyles {
        item,
        selected,
        text: theme.text_primary,
        selected_text: theme.accent.readable_text(),
    }
}

macro_rules! impl_application_widget {
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

impl_application_widget!(Accordion, Field, Form, Pagination);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themed_application_parts_mount_and_use_theme() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(Theme::light()));
            let slot = || Box::new(RawView::new(Style::default())) as BoxedWidget;
            let mut accordion = Accordion::new(|_, _| {}).item("Section", slot(), true);
            assert_eq!(accordion.children().len(), 1);
            let mut field = Field::new("Email", slot(), true)
                .hint("Optional hint")
                .error("Invalid");
            assert_eq!(field.children().len(), 3);
            let mut form = Form::new()
                .field(slot())
                .submit("Save", || {})
                .submitting(true);
            assert!(form.children()[1].on_click().is_none());
            let mut pagination = Pagination::new(2, 5, |_| {});
            assert_eq!(pagination.children().len(), 7);
        });
    }
}
