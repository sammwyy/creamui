use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::{RawKbd, RawTag};

pub struct Kbd {
    inner: RawKbd,
}
impl_styled_inner!(Kbd);

impl Kbd {
    pub fn new(keys: impl IntoIterator<Item = String>) -> Self {
        let theme = use_theme();
        Self {
            inner: RawKbd::new(
                Style::default().padding_xy(theme.spacing_medium, theme.spacing_small),
                keys,
            )
            .text_style(theme.text_primary, theme.typography.caption)
            .background(elevation::control_fill(&theme))
            .border(elevation::control_border(&theme), 1.0)
            .box_shadow(creamui_core::BoxShadow::new(
                0.0,
                1.0,
                0.0,
                0.0,
                elevation::control_border(&theme),
            ))
            .corner_radius(theme.radius_small - 2.0),
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
                    .gap(theme.spacing_medium)
                    .padding_xy(
                        theme.spacing_medium + theme.spacing_small,
                        theme.spacing_small + 2.0,
                    ),
                Box::new(RawText::new(
                    label.clone(),
                    theme.text_primary,
                    theme.typography.caption,
                )),
            )
            .background(elevation::with_alpha(theme.text_primary, 12))
            .border(elevation::with_alpha(theme.text_primary, 18), 1.0)
            .corner_radius(999.0),
            label,
        }
    }

    pub fn selected(mut self, selected: bool, on_select: impl Fn(bool) + 'static) -> Self {
        let theme = use_theme();
        self.inner = self.inner.selectable(selected, on_select);
        if selected {
            self.inner = self
                .inner
                .background(elevation::with_alpha(theme.accent, 44))
                .border(elevation::with_alpha(theme.accent, 140), 1.0)
                .label(Box::new(
                    RawText::new(
                        self.label.clone(),
                        elevation::tinted_text(&theme, theme.accent),
                        theme.typography.caption,
                    )
                    .bold(true),
                ));
        }
        self
    }

    pub fn removable(mut self, on_remove: impl Fn() + 'static) -> Self {
        let theme = use_theme();
        self.inner = self.inner.removable(
            Box::new(crate::Icon::new(crate::Symbol::Close, theme.text_secondary).size(12.0)),
            on_remove,
        );
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

impl_content_widget!(Kbd, Tag);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themed_content_slots_are_mounted() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            assert_eq!(Kbd::new(["Mod".into(), "S".into()]).children().len(), 1);
            assert_eq!(Tag::new("Review").removable(|| {}).children().len(), 2);
        });
    }

    #[test]
    fn compact_content_has_readable_insets_and_consistent_border() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let tag = Tag::new("Review");
            assert_eq!(
                tag.style().layout.padding.left,
                creamui_core::layout::LengthPercentage::Length(12.0)
            );
            assert!(tag.style().paint.border.is_some());
            let selected = Tag::new("Review").selected(true, |_| {});
            assert!(selected.style().paint.border.is_some());
            let kbd = Kbd::new(["Mod".into()]);
            assert_eq!(
                kbd.style().layout.padding.left,
                creamui_core::layout::LengthPercentage::Length(8.0)
            );
        });
    }
}
