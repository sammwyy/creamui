use super::*;
use crate::{ContextMenuController, ContextMenuStyle, RawContextMenu};
use creamui_core::Size;

pub struct ContextMenu {
    inner: RawContextMenu,
}
impl ContextMenu {
    pub fn new(controller: &ContextMenuController, viewport: Size) -> Self {
        let theme = use_theme();
        Self {
            inner: RawContextMenu::new(
                controller,
                viewport,
                ContextMenuStyle {
                    background: theme.surface_elevated,
                    border: theme.border,
                    text: theme.text_primary,
                    muted: theme.text_disabled,
                    active: theme.surface_hover,
                    radius: theme.menu_radius,
                    item_radius: theme.menu_item_radius,
                    row_height: 32.0 * theme.text_scale,
                    font_size: 13.0 * theme.text_scale,
                    width: 248.0 * theme.text_scale,
                    shadow: Some(creamui_core::BoxShadow::new(
                        0.0,
                        6.0,
                        18.0,
                        0.0,
                        Color::rgba(0, 0, 0, 48),
                    )),
                },
            ),
        }
    }
    pub fn pane(controller: &ContextMenuController, path: Vec<usize>, viewport: Size) -> Self {
        let mut menu = Self::new(controller, viewport);
        menu.inner = RawContextMenu::pane(controller, path, viewport, menu.inner.tokens());
        menu
    }
    pub fn pane_size(
        controller: &ContextMenuController,
        path: &[usize],
        theme: &Theme,
    ) -> Option<Size> {
        Some(Size {
            width: 248.0 * theme.text_scale,
            height: controller.pane_height(path, 32.0 * theme.text_scale)?,
        })
    }
}
impl Widget for ContextMenu {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect)
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
    fn is_modal(&self) -> bool {
        self.inner.is_modal()
    }
    fn blocks_pointer(&self) -> bool {
        true
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }
    fn on_context_menu(&self) -> Option<Rc<dyn Fn(Point)>> {
        self.inner.on_context_menu()
    }
    fn focusable(&self) -> bool {
        true
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }
}
