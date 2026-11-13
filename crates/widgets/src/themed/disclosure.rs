use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::{AlertStatus, RawAlert, RawCollapsible, RawEmptyState, RawSkeleton};

pub struct Collapsible {
    inner: RawCollapsible,
}
impl_styled_inner!(Collapsible);

impl Collapsible {
    pub fn new(
        label: impl Into<String>,
        expanded: bool,
        on_toggle: impl Fn(bool) + 'static,
    ) -> Self {
        let theme = use_theme();
        let trigger = RawView::new(
            Style::default()
                .flex_row()
                .gap(theme.spacing_medium)
                .align(Align::Center)
                .padding_xy(
                    theme.spacing_large,
                    theme.spacing_medium + theme.spacing_small,
                ),
        )
        .corner_radius(theme.radius_medium)
        .hover_style(
            creamui_core::StateStyle::new()
                .background(elevation::with_alpha(theme.text_primary, 12)),
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
            RawText::new(label, theme.text_primary, theme.typography.body)
                .font_family(theme.font_family),
        ));
        Self {
            inner: RawCollapsible::new(Style::default(), expanded, on_toggle)
                .background(elevation::group_fill(&theme))
                .border(theme.border, 1.0)
                .corner_radius(theme.radius_medium)
                .trigger(Box::new(trigger)),
        }
    }

    pub fn content(mut self, content: BoxedWidget) -> Self {
        let theme = use_theme();
        self.inner = self.inner.content(Box::new(
            RawView::new(
                Style::default()
                    .flex_column()
                    .padding_xy(theme.spacing_large, theme.spacing_medium),
            )
            .child(content),
        ));
        self
    }
}

impl Widget for Collapsible {
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlertVariant {
    #[default]
    Subtle,
    Solid,
}

pub struct Alert {
    inner: RawAlert,
    foreground: Color,
}
impl_styled_inner!(Alert);

impl Alert {
    pub fn new(status: AlertStatus, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::styled(status, AlertVariant::Subtle, title, message)
    }

    pub fn styled(
        status: AlertStatus,
        variant: AlertVariant,
        title: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let theme = use_theme();
        let tone = match status {
            AlertStatus::Info => theme.accent,
            AlertStatus::Success => theme.success,
            AlertStatus::Warning => theme.warning,
            AlertStatus::Error => theme.danger,
        };
        let light = elevation::is_light(&theme);
        let solid = variant == AlertVariant::Solid;
        let (background, text, secondary) = if solid {
            let text = tone.readable_text();
            (tone, text, elevation::with_alpha(text, 215))
        } else {
            (
                elevation::tint(&theme, tone),
                theme.text_primary,
                theme.text_secondary,
            )
        };
        let badge = if solid {
            elevation::with_alpha(text, 40)
        } else {
            elevation::with_alpha(tone, if light { 40 } else { 56 })
        };
        let glyph = if solid {
            text
        } else {
            elevation::tinted_text(&theme, tone)
        };
        let symbol: BoxedWidget = match status {
            AlertStatus::Success => {
                Box::new(crate::Icon::new(crate::Symbol::Check, glyph).size(16.0))
            }
            AlertStatus::Error => {
                Box::new(crate::Icon::new(crate::Symbol::Close, glyph).size(14.0))
            }
            AlertStatus::Info | AlertStatus::Warning => Box::new(
                RawText::new(
                    if status == AlertStatus::Info {
                        "i"
                    } else {
                        "!"
                    },
                    glyph,
                    theme.typography.body,
                )
                .bold(true),
            ),
        };
        let icon = RawView::new(
            Style::default()
                .flex_row()
                .align(Align::Center)
                .justify(crate::layout::Justify::Center)
                .size(26.0, 26.0),
        )
        .corner_radius(13.0)
        .background(badge)
        .child(symbol);
        let inner = RawAlert::new(
            Style::default()
                .flex_row()
                .align(Align::Center)
                .gap(theme.spacing_medium + theme.spacing_small)
                .padding_xy(
                    theme.spacing_large,
                    theme.spacing_medium + theme.spacing_small,
                ),
            status,
        )
        .background(background)
        .border(
            if solid {
                tone
            } else {
                elevation::with_alpha(tone, if light { 56 } else { 48 })
            },
            1.0,
        )
        .corner_radius(theme.radius_medium)
        .icon(Box::new(icon))
        .title(Box::new(
            RawText::new(title, text, theme.typography.body)
                .bold(true)
                .font_family(theme.font_family),
        ))
        .message(Box::new(
            RawText::new(message, secondary, theme.typography.body)
                .font_family(theme.font_family)
                .margin_top(2.0),
        ));
        Self {
            inner,
            foreground: if solid { text } else { theme.text_secondary },
        }
    }

    pub fn action(mut self, action: BoxedWidget) -> Self {
        self.inner = self.inner.action(action);
        self
    }

    pub fn dismiss(mut self, on_dismiss: impl Fn() + 'static) -> Self {
        self.inner = self.inner.dismiss(
            Box::new(crate::Icon::new(crate::Symbol::Close, self.foreground).size(14.0)),
            on_dismiss,
        );
        self
    }
}

impl Widget for Alert {
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

pub struct Skeleton {
    inner: RawSkeleton,
}
impl_styled_inner!(Skeleton);

impl Skeleton {
    pub fn block(width: f32, height: f32) -> Self {
        let theme = use_theme();
        Self {
            inner: RawSkeleton::new(Style::default().size(width.max(0.0), height.max(0.0)))
                .background(theme.surface_hover)
                .corner_radius(theme.radius_small),
        }
    }

    pub fn circle(diameter: f32) -> Self {
        let diameter = diameter.max(0.0);
        let mut skeleton = Self::block(diameter, diameter);
        skeleton.inner = skeleton.inner.corner_radius(diameter / 2.0);
        skeleton
    }
}

impl Widget for Skeleton {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }
}

pub struct EmptyState {
    inner: RawEmptyState,
}
impl_styled_inner!(EmptyState);

impl EmptyState {
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        let theme = use_theme();
        Self {
            inner: RawEmptyState::new(
                Style::default()
                    .flex_column()
                    .align(Align::Center)
                    .gap(theme.spacing_medium)
                    .padding_all(theme.spacing_large + theme.spacing_medium),
                Box::new(
                    RawText::new(title, theme.text_primary, theme.typography.section)
                        .bold(true)
                        .font_family(theme.font_family)
                        .text_align(TextAlign::Center),
                ),
            )
            .description(Box::new(
                RawText::new(description, theme.text_secondary, theme.typography.body)
                    .font_family(theme.font_family)
                    .text_align(TextAlign::Center),
            )),
        }
    }

    pub fn media(mut self, media: BoxedWidget) -> Self {
        self.inner = self.inner.media(media);
        self
    }

    pub fn action(mut self, action: BoxedWidget) -> Self {
        self.inner = self.inner.action(Box::new(
            RawView::new(Style::default().margin_xy(0.0, use_theme().spacing_medium)).child(action),
        ));
        self
    }

    pub fn compact(mut self) -> Self {
        self.inner.style.layout.padding = Style::default()
            .padding_all(use_theme().spacing_small)
            .padding;
        self
    }
}

impl Widget for EmptyState {
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
    fn themed_composites_preserve_slots() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let mut collapsible = Collapsible::new("Details", true, |_| {})
                .content(Box::new(RawView::new(Style::default())));
            assert_eq!(collapsible.children().len(), 2);

            let mut alert = Alert::new(AlertStatus::Info, "Title", "Message").dismiss(|| {});
            assert_eq!(alert.children().len(), 3);

            let mut empty = EmptyState::new("Nothing here", "Try again")
                .action(Box::new(RawView::new(Style::default())));
            assert_eq!(empty.children().len(), 3);

            assert_eq!(
                Skeleton::circle(28.0).style().paint.corner_radius,
                Some(14.0)
            );
        });
    }

    #[test]
    fn theme_colors_and_solid_warning_contrast() {
        for theme in [Theme::light(), Theme::dark()] {
            creamui_reactive::with_context_scope(|| {
                creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme.clone()));
                let skeleton = Skeleton::block(80.0, 16.0);
                assert_eq!(
                    skeleton.style().paint.background,
                    Some(theme.surface_hover.into())
                );
                let mut alert = Alert::styled(
                    AlertStatus::Warning,
                    AlertVariant::Solid,
                    "Warning",
                    "Careful",
                );
                assert_eq!(alert.style().paint.background, Some(theme.warning.into()));
                let children = alert.children();
                assert_eq!(children.len(), 2);
            });
        }
    }

    #[test]
    fn disclosure_and_alert_have_themed_containers_and_separate_status_icon() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let theme = use_theme();
            let mut collapsible = Collapsible::new("Details", false, |_| {});
            assert_eq!(
                collapsible.style().paint.corner_radius,
                Some(theme.radius_medium)
            );
            assert!(collapsible.style().paint.border.is_some());
            let mut trigger = collapsible.children();
            let trigger_content = trigger[0].children();
            assert_eq!(
                trigger_content[0].style().layout.padding.left,
                creamui_core::layout::LengthPercentage::Length(theme.spacing_large)
            );

            let mut alert = Alert::new(AlertStatus::Info, "Notice", "Description");
            assert!(alert.style().paint.border.is_some());
            let mut children = alert.children();
            assert_eq!(
                children[0].style().layout.size.width,
                Dimension::Length(26.0)
            );
            assert_eq!(children[0].children().len(), 1);
        });
    }
}
