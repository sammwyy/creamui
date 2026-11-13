use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::{RawAccordion, RawField, RawForm, RawPagination};

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
        let header = RawView::new(
            Style::default()
                .flex_row()
                .gap(theme.spacing_small)
                .align(Align::Center)
                .padding_all(theme.spacing_medium),
        )
        .background(theme.surface_elevated)
        .corner_radius(theme.radius_medium)
        .child(Box::new(RawText::new(
            if expanded { "−" } else { "+" },
            theme.accent,
            theme.typography.body,
        )))
        .child(Box::new(
            RawText::new(heading, theme.text_primary, theme.typography.body)
                .font_family(theme.font_family),
        ));
        self.inner = self.inner.item(Box::new(header), content, expanded);
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
                Box::new(RawText::new(
                    label,
                    theme.selection_text,
                    theme.typography.body,
                )),
                on_submit,
            )
            .submit_style(
                creamui_core::Style::new()
                    .background(theme.accent)
                    .corner_radius(theme.button_radius)
                    .padding(theme.spacing_medium),
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
            .colors(theme.surface_elevated, theme.text_primary, theme.accent),
        }
    }

    pub fn compact(mut self) -> Self {
        self.inner = self.inner.compact();
        self
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
