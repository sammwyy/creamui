use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::{RawTooltip, TooltipController, TooltipPosition};

pub struct Tooltip {
    inner: RawTooltip,
    text: String,
}
impl_styled_inner!(Tooltip);

impl Tooltip {
    pub fn new(
        controller: &TooltipController,
        trigger: BoxedWidget,
        text: impl Into<String>,
    ) -> Self {
        let theme = use_theme();
        let text = text.into();
        let content = RawView::new(Style::default().flex_column().padding_xy(
            theme.spacing_medium + theme.spacing_small,
            theme.spacing_medium,
        ))
        .background(elevation::overlay_fill(&theme))
        .border(elevation::overlay_border(&theme), 1.0)
        .corner_radius(theme.radius_small)
        .box_shadow(elevation::Elevation::Overlay.shadow(&theme))
        .child(Box::new(RawText::new(
            text.clone(),
            theme.text_primary,
            theme.typography.caption,
        )));
        Self {
            inner: RawTooltip::new(
                Style::default().flex_column().align(Align::Start),
                controller,
                trigger,
                Box::new(content),
            )
            .popup_size(220.0, 44.0),
            text,
        }
    }

    pub fn position(mut self, position: TooltipPosition) -> Self {
        self.inner = self.inner.position(position);
        self
    }

    pub fn shortcut(mut self, label: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.popup_size(240.0, 64.0);
        let content = RawView::new(
            Style::default()
                .flex_column()
                .gap(theme.spacing_small)
                .padding_xy(
                    theme.spacing_medium + theme.spacing_small,
                    theme.spacing_medium,
                ),
        )
        .background(elevation::overlay_fill(&theme))
        .border(elevation::overlay_border(&theme), 1.0)
        .corner_radius(theme.radius_small)
        .box_shadow(elevation::Elevation::Overlay.shadow(&theme))
        .child(Box::new(RawText::new(
            self.text.clone(),
            theme.text_primary,
            theme.typography.caption,
        )))
        .child(Box::new(RawText::new(
            label,
            theme.text_secondary,
            theme.typography.caption,
        )));
        self.inner = self.inner.content(Box::new(content));
        self
    }
}

impl Widget for Tooltip {
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
    fn themed_tooltip_mounts_trigger_and_popup_on_focus() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let controller = TooltipController::default();
            let mut tooltip = Tooltip::new(
                &controller,
                Box::new(RawText::new("Help", Color::rgb(0, 0, 0), 14.0)),
                "Shows a hint",
            );
            assert_eq!(tooltip.children().len(), 1);
            controller.focus(true);
            let mut tooltip = Tooltip::new(
                &controller,
                Box::new(RawText::new("Help", Color::rgb(0, 0, 0), 14.0)),
                "Shows a hint",
            );
            assert_eq!(tooltip.children().len(), 2);
        });
    }

    #[test]
    fn tooltip_popup_has_opaque_fill_and_room_for_shortcut() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let controller = TooltipController::default();
            controller.focus(true);
            let mut tooltip = Tooltip::new(
                &controller,
                Box::new(RawText::new("Help", Color::rgb(0, 0, 0), 14.0)),
                "Shows a hint",
            )
            .shortcut("Ctrl+H");
            let mut slots = tooltip.children();
            let mut popup = slots.pop().unwrap();
            let content = popup.children();
            assert!(content[0].style().paint.border.is_some());
            assert_eq!(
                content[0].style().layout.padding.left,
                creamui_core::layout::LengthPercentage::Length(12.0)
            );
            assert_eq!(
                content[0].style().paint.background,
                Some(elevation::overlay_fill(&use_theme()).into())
            );
        });
    }
}
