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
                    .background(elevation::overlay_fill(&theme))
                    .border(elevation::overlay_border(&theme), 1.0)
                    .box_shadow(elevation::Elevation::Modal.shadow(&theme))
                    .style(),
            )
            .scrim(elevation::scrim(&theme));
        Self { inner, dismiss }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        let theme = use_theme();
        let dismiss = self.dismiss.clone();
        self.inner = self.inner.header(Box::new(
            RawView::new(Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(56.0),
                },
                flex_shrink: 0.0,
                ..Style::default()
                    .flex_row()
                    .align(Align::Center)
                    .gap(theme.spacing_medium)
                    .padding_xy(theme.spacing_large, theme.spacing_medium)
            })
            .child(Box::new(
                RawText::new(title, theme.text_primary, theme.typography.section)
                    .bold(true)
                    .layout(Style {
                        flex_grow: 1.0,
                        ..Default::default()
                    }),
            ))
            .child(Box::new(
                RawButton::new(
                    Style::default()
                        .flex_row()
                        .align(Align::Center)
                        .justify(crate::layout::Justify::Center)
                        .size(32.0, 32.0),
                    move || dismiss(),
                )
                .corner_radius(theme.button_radius)
                .hover_style(
                    creamui_core::StateStyle::new()
                        .background(elevation::with_alpha(theme.text_primary, 16)),
                )
                .focus_style(
                    creamui_core::StateStyle::new().outline(elevation::focus_ring(&theme), 2.0),
                )
                .child(Box::new(
                    crate::Icon::new(crate::Symbol::Close, theme.text_secondary).size(16.0),
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

    #[test]
    fn sheet_header_has_a_padded_close_target() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let mut sheet = Sheet::new(
                SheetEdge::Bottom,
                ScrollController::new(0.0),
                Box::new(RawView::new(Style::default())),
                || {},
            )
            .title("Details");
            let mut panel = sheet.children();
            let mut content = panel[0].children();
            let mut header = content.remove(0);
            assert_eq!(header.style().layout.size.height, Dimension::Length(56.0));
            let children = header.children();
            assert_eq!(
                children[1].style().layout.size.width,
                Dimension::Length(32.0)
            );
            assert!(children[1].focusable());
        });
    }
}
