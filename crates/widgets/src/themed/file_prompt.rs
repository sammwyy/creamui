use super::*;
use crate::layout::{column, padding, row};
use crate::FilePickerController;
use creamui_core::layout::Display;
use creamui_core::Key;

/// A file path modal placed after application content at the window root.
/// `FilePickerController::host` includes and provides it automatically.
pub struct FilePrompt {
    inner: Option<Overlay>,
}

impl FilePrompt {
    pub fn new(controller: &FilePickerController) -> Self {
        let Some(request) = controller.request.get() else {
            return Self { inner: None };
        };
        let theme = use_theme();
        let submit = controller.clone();
        let escape = controller.clone();
        let input = TextInput::controlled_with_style(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(40.0),
                },
                flex_shrink: 0.0,
                ..TextInput::default_style()
            },
            &controller.path(),
        )
        .placeholder("File path")
        .on_submit(move || submit.submit())
        .on_key_press(move |input| {
            if input.key == Key::Escape {
                escape.cancel();
            }
        });
        let mut card = Popover::new(padding(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Auto,
                },
                max_size: creamui_core::layout::Size {
                    width: Dimension::Length(420.0),
                    height: Dimension::Auto,
                },
                ..column(theme.spacing_medium)
            },
            theme.spacing_large,
        ))
        .child(Box::new(Heading::md(request.title.clone())))
        .child(Box::new(Text::secondary(
            "Enter a path to an accessible local file.",
        )))
        .child(Box::new(input));
        if !request.filters.is_empty() {
            let filters = request
                .filters
                .iter()
                .map(|(name, extensions)| format!("{name}: {}", extensions.join(", ")))
                .collect::<Vec<_>>()
                .join("; ");
            card = card.child(Box::new(Text::secondary(filters)));
        }
        if let Some(error) = controller.error() {
            card = card.child(Box::new(RawText::new(error, theme.danger, 12.0)));
        }
        let cancel = controller.clone();
        let submit = controller.clone();
        let actions = RawView::new(Style {
            justify_content: Some(JustifyContent::End),
            ..row(theme.spacing_small)
        })
        .child(Box::new(
            Button::styled(
                ButtonVariant::Secondary,
                ButtonSize::Md,
                "Cancel",
                ButtonState::Normal,
                move || cancel.cancel(),
            )
            .on_key_press(dismiss_on_escape(controller.clone())),
        ))
        .child(Box::new(
            Button::styled(
                ButtonVariant::Primary,
                ButtonSize::Md,
                "Open",
                ButtonState::Normal,
                move || submit.submit(),
            )
            .on_key_press(dismiss_on_escape(controller.clone())),
        ));
        let cancel = controller.clone();
        let overlay = Overlay::new(
            padding(
                Overlay::fullscreen(|| {}).style().layout,
                theme.spacing_medium,
            ),
            move || cancel.cancel(),
        )
        .child(Box::new(card.child(Box::new(actions))));
        Self {
            inner: Some(overlay),
        }
    }
}

fn dismiss_on_escape(controller: FilePickerController) -> impl Fn(KeyInput) {
    move |input| {
        if input.key == Key::Escape {
            controller.cancel();
        }
    }
}

impl Widget for FilePrompt {
    fn is_modal(&self) -> bool {
        self.inner.is_some()
    }
    fn style(&self) -> creamui_core::Style {
        self.inner.as_ref().map_or_else(
            || {
                Style {
                    display: Display::None,
                    ..Default::default()
                }
                .into()
            },
            Widget::style,
        )
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(inner) = &self.inner {
            inner.paint(painter, rect);
        }
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.as_mut().map_or_else(Vec::new, Widget::children)
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.as_ref().and_then(Widget::on_click)
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.as_ref().and_then(Widget::cursor_icon)
    }
}
