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
                    .padding_all(theme.spacing_medium)
            },
            Box::new(body),
        )
        .background(theme.surface_elevated)
        .border(status, 1.5)
        .corner_radius(theme.card_radius)
        .dismiss(
            Box::new(RawText::new(
                "x",
                theme.text_secondary,
                theme.typography.caption,
            )),
            move || dismiss(),
        );
        if let Some((label, action)) = entry.action {
            toast = toast.action(
                Box::new(RawText::new(label, status, theme.typography.caption)),
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
