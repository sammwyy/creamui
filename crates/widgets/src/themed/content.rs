use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::{RawAttachment, RawChatMessage, RawKbd, RawTag};

pub struct Kbd {
    inner: RawKbd,
}
impl_styled_inner!(Kbd);

impl Kbd {
    pub fn new(keys: impl IntoIterator<Item = String>) -> Self {
        let theme = use_theme();
        Self {
            inner: RawKbd::new(Style::default().padding_xy(theme.spacing_small, 4.0), keys)
                .text_style(theme.text_primary, theme.typography.caption)
                .background(theme.surface_hover)
                .border(theme.border, 1.0)
                .corner_radius(theme.radius_small),
        }
    }
}

pub struct Tag {
    inner: RawTag,
    label: String,
}
impl_styled_inner!(Tag);

impl Tag {
    pub fn new(label: impl Into<String>) -> Self {
        let theme = use_theme();
        let label = label.into();
        Self {
            inner: RawTag::new(
                Style::default()
                    .flex_row()
                    .align(Align::Center)
                    .gap(theme.spacing_small)
                    .padding_xy(theme.spacing_small, 4.0),
                Box::new(RawText::new(
                    label.clone(),
                    theme.text_primary,
                    theme.typography.caption,
                )),
            )
            .background(theme.surface_elevated)
            .corner_radius(theme.radius_medium),
            label,
        }
    }

    pub fn selected(mut self, selected: bool, on_select: impl Fn(bool) + 'static) -> Self {
        let theme = use_theme();
        self.inner = self.inner.selectable(selected, on_select);
        if selected {
            self.inner = self
                .inner
                .background(theme.accent)
                .label(Box::new(RawText::new(
                    self.label.clone(),
                    theme.selection_text,
                    theme.typography.caption,
                )));
        }
        self
    }

    pub fn removable(mut self, on_remove: impl Fn() + 'static) -> Self {
        let theme = use_theme();
        self.inner = self.inner.removable(
            Box::new(RawText::new(
                "×",
                theme.text_secondary,
                theme.typography.body,
            )),
            on_remove,
        );
        self
    }
}

pub struct Attachment {
    inner: RawAttachment,
}
impl_styled_inner!(Attachment);

impl Attachment {
    pub fn new(name: impl Into<String>) -> Self {
        let theme = use_theme();
        Self {
            inner: RawAttachment::new(
                Style::default()
                    .flex_row()
                    .align(Align::Center)
                    .gap(theme.spacing_medium)
                    .padding_all(theme.spacing_medium),
                Box::new(RawText::new(
                    name,
                    theme.text_primary,
                    theme.typography.body,
                )),
            )
            .background(theme.surface_elevated)
            .corner_radius(theme.card_radius),
        }
    }

    pub fn preview(mut self, preview: BoxedWidget) -> Self {
        self.inner = self.inner.preview(preview);
        self
    }

    pub fn progress(mut self, value: f32) -> Self {
        self.inner = self.inner.progress(Box::new(ProgressBar::new(value)));
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

    pub fn action(mut self, action: BoxedWidget) -> Self {
        self.inner = self.inner.action(action);
        self
    }
}

pub struct ChatMessage {
    inner: RawChatMessage,
}
impl_styled_inner!(ChatMessage);

impl ChatMessage {
    pub fn new(sender: impl Into<String>, body: BoxedWidget, outgoing: bool) -> Self {
        let theme = use_theme();
        Self {
            inner: RawChatMessage::new(
                Style::default()
                    .flex_column()
                    .gap(theme.spacing_small)
                    .padding_all(theme.spacing_medium),
                Box::new(
                    RawText::new(sender, theme.text_primary, theme.typography.body).bold(true),
                ),
                body,
            )
            .background(if outgoing {
                theme.accent.mix(theme.surface, 0.7)
            } else {
                theme.surface_elevated
            })
            .corner_radius(theme.card_radius),
        }
    }

    pub fn timestamp(mut self, timestamp: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.timestamp(Box::new(RawText::new(
            timestamp,
            theme.text_secondary,
            theme.typography.caption,
        )));
        self
    }

    pub fn action(mut self, action: BoxedWidget) -> Self {
        self.inner = self.inner.action(action);
        self
    }
}

macro_rules! impl_content_widget {
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

impl_content_widget!(Kbd, Tag, Attachment, ChatMessage);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themed_content_slots_are_mounted() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let slot = || Box::new(RawView::new(Style::default())) as BoxedWidget;
            assert_eq!(Kbd::new(["Mod".into(), "S".into()]).children().len(), 1);
            assert_eq!(Tag::new("Review").removable(|| {}).children().len(), 2);
            let mut attachment = Attachment::new("notes.txt").progress(0.5).action(slot());
            assert_eq!(attachment.children().len(), 2);
            let mut chat = ChatMessage::new("Sam", slot(), false).timestamp("12:30");
            assert_eq!(chat.children().len(), 2);
        });
    }
}
