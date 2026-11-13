use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::{RawToast, RawToastHost, ToastController, ToastEntry, ToastStatus};
use creamui_core::layout::{LengthPercentageAuto, Position};

pub struct Toast {
    inner: RawToast,
}
impl_styled_inner!(Toast);

impl Toast {
    pub fn new(entry: ToastEntry, on_dismiss: impl Fn() + 'static) -> Self {
        Self::with_theme(entry, Rc::new(on_dismiss), use_theme())
    }

    fn with_theme(entry: ToastEntry, on_dismiss: Rc<dyn Fn()>, theme: Theme) -> Self {
        let status = match entry.status {
            ToastStatus::Info => theme.accent,
            ToastStatus::Success => theme.success,
            ToastStatus::Warning => theme.warning,
            ToastStatus::Error => theme.danger,
        };
        let body = RawView::new(Style {
            flex_grow: 1.0,
            min_size: creamui_core::layout::Size {
                width: Dimension::Length(0.0),
                height: Dimension::Auto,
            },
            ..Style::default().flex_column().gap(theme.spacing_small)
        })
        .child(Box::new(
            RawText::new(entry.title, theme.text_primary, theme.typography.body).bold(true),
        ))
        .child(Box::new(RawText::new(
            entry.message,
            theme.text_secondary,
            theme.typography.caption,
        )));
        let indicator = RawView::new(Style {
            size: crate::layout::fixed(8.0, 8.0),
            flex_shrink: 0.0,
            ..Default::default()
        })
        .background(status)
        .corner_radius(4.0);
        let body = RawView::new(Style {
            flex_grow: 1.0,
            min_size: creamui_core::layout::Size {
                width: Dimension::Length(0.0),
                height: Dimension::Auto,
            },
            ..Style::default()
                .flex_row()
                .align(Align::Center)
                .gap(theme.spacing_medium + theme.spacing_small)
        })
        .child(Box::new(indicator))
        .child(Box::new(body));
        let dismiss = on_dismiss.clone();
        let mut toast = RawToast::new(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Auto,
                },
                ..Style::default()
                    .flex_row()
                    .align(Align::Center)
                    .gap(theme.spacing_medium)
                    .padding_xy(theme.spacing_large, theme.spacing_medium + 4.0)
            },
            Box::new(body),
        )
        .background(elevation::overlay_fill(&theme))
        .border(elevation::overlay_border(&theme), 1.0)
        .corner_radius(theme.card_radius)
        .box_shadow(elevation::Elevation::Overlay.shadow(&theme))
        .dismiss(
            Box::new(crate::Icon::new(crate::Symbol::Close, theme.text_secondary).size(14.0)),
            move || dismiss(),
        );
        if let Some((label, action)) = entry.action {
            toast = toast.action(
                Box::new(
                    RawText::new(
                        label,
                        elevation::tinted_text(&theme, status),
                        theme.typography.caption,
                    )
                    .bold(true),
                ),
                move || {
                    action();
                    on_dismiss();
                },
            );
        }
        Self { inner: toast }
    }
}

impl Widget for Toast {
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

pub struct ToastHost {
    inner: RawToastHost,
}
impl_styled_inner!(ToastHost);

impl ToastHost {
    pub fn new(controller: &ToastController) -> Self {
        let theme = use_theme();
        Self {
            inner: RawToastHost::new(
                Style {
                    position: Position::Absolute,
                    inset: creamui_core::layout::Rect {
                        left: LengthPercentageAuto::Auto,
                        right: LengthPercentageAuto::Length(16.0),
                        top: LengthPercentageAuto::Length(16.0),
                        bottom: LengthPercentageAuto::Auto,
                    },
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(320.0),
                        height: Dimension::Auto,
                    },
                    max_size: creamui_core::layout::Size {
                        width: Dimension::Percent(0.85),
                        height: Dimension::Auto,
                    },
                    ..Style::default().flex_column().gap(theme.spacing_small)
                },
                controller,
                move |entry, dismiss| Box::new(Toast::with_theme(entry, dismiss, theme.clone())),
            ),
        }
    }
}

impl Widget for ToastHost {
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
    use std::time::Duration;

    #[test]
    fn themed_host_stacks_and_exposes_dismiss_actions() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let controller = ToastController::new();
            controller.push(ToastEntry::new(
                ToastStatus::Success,
                "Saved",
                "Your changes are ready.",
                Some(Duration::from_secs(10)),
            ));
            let mut host = ToastHost::new(&controller);
            let mut toasts = host.children();
            assert_eq!(toasts.len(), 1);
            assert_eq!(toasts[0].children().len(), 2);
        });
    }
}
