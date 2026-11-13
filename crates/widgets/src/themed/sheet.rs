use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::{RawSheet, SheetEdge};
use crate::ScrollController;

pub struct Sheet {
    inner: RawSheet,
    dismiss: Rc<dyn Fn()>,
}
impl_styled_inner!(Sheet);

impl Sheet {
    pub fn new(
        edge: SheetEdge,
        scroll: ScrollController,
        content: BoxedWidget,
        on_dismiss: impl Fn() + 'static,
    ) -> Self {
        let theme = use_theme();
        let dismiss: Rc<dyn Fn()> = Rc::new(on_dismiss);
        let callback = dismiss.clone();
        let inner = RawSheet::new(Style::default(), scroll, edge, content, move || callback())
            .panel_style(
                RawView::new(Style::default().flex_column())
                    .background(theme.surface_elevated)
                    .border(theme.border, 1.0)
                    .style(),
            )
            .scrim(Color::rgba(0, 0, 0, 110));
        Self { inner, dismiss }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        let theme = use_theme();
        let dismiss = self.dismiss.clone();
        self.inner =
            self.inner.header(Box::new(
                RawView::new(Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(50.0),
                    },
                    flex_shrink: 0.0,
                    ..Style::default()
                        .flex_row()
                        .align(Align::Center)
                        .gap(theme.spacing_medium)
                        .padding_all(theme.spacing_medium)
                })
                .child(Box::new(
                    RawText::new(title, theme.text_primary, theme.typography.body)
                        .bold(true)
                        .layout(Style {
                            flex_grow: 1.0,
                            ..Default::default()
                        }),
                ))
                .child(Box::new(
                    RawButton::new(Style::default(), move || dismiss()).child(Box::new(
                        RawText::new("x", theme.text_secondary, theme.typography.body),
                    )),
                )),
            ));
        self
    }

    pub fn extent(mut self, extent: f32) -> Self {
        self.inner = self.inner.extent(extent);
        self
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.inner = self.inner.modal(modal);
        self
    }
}

impl Widget for Sheet {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn is_modal(&self) -> bool {
        self.inner.is_modal()
    }

    fn blocks_pointer(&self) -> bool {
        self.inner.blocks_pointer()
    }

    fn on_modal_dismiss(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_modal_dismiss()
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
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
    fn themed_sheet_keeps_modal_and_panel_slots() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let mut sheet = Sheet::new(
                SheetEdge::Bottom,
                ScrollController::new(0.0),
                Box::new(RawView::new(Style::default())),
                || {},
            )
            .title("Details");
            assert!(sheet.is_modal());
            assert_eq!(sheet.children().len(), 1);
        });
    }
}
