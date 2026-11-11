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
        let content = RawView::new(
            Style::default()
                .flex_column()
                .padding_all(theme.spacing_small),
        )
        .background(theme.surface_elevated)
        .border(theme.border, 1.0)
        .corner_radius(theme.radius_small)
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
            .popup_size(200.0, 42.0),
            text,
        }
    }

    pub fn position(mut self, position: TooltipPosition) -> Self {
        self.inner = self.inner.position(position);
        self
    }

    pub fn shortcut(mut self, label: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.popup_size(220.0, 50.0);
        self.inner = self.inner.content(Box::new(
            RawView::new(
                Style::default()
                    .flex_column()
                    .padding_all(theme.spacing_small),
            )
            .background(theme.surface_elevated)
            .border(theme.border, 1.0)
            .corner_radius(theme.radius_small)
            .child(Box::new(RawText::new(
                self.text.clone(),
                theme.text_primary,
                theme.typography.caption,
            )))
            .child(Box::new(RawText::new(
                label,
                theme.text_secondary,
                theme.typography.caption,
            ))),
        ));
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
}
