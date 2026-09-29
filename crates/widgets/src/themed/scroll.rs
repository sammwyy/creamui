use super::*;
use crate::{RawVirtualList, ScrollController, VirtualListState};

fn with_alpha(color: Color, alpha: u8) -> Color {
    Color::rgba(color.r, color.g, color.b, alpha)
}

/// A themed vertically-scrollable container. When built via
/// [`ScrollView::controlled`], its scrollbar (see [`RawScrollView`]'s doc
/// comment) is colored from the theme instead of `RawScrollView`'s neutral
/// gray default.
pub struct ScrollView {
    inner: RawScrollView,
}
impl_styled_inner!(ScrollView);

impl ScrollView {
    pub fn new(style: Style, scroll_y: f32, on_scroll: impl Fn(f32) + 'static) -> Self {
        let theme = use_theme();
        let inner = RawScrollView::new(style, scroll_y, on_scroll)
            .background(theme.surface)
            .corner_radius(theme.radius_medium);
        ScrollView { inner }
    }

    pub fn controlled(style: Style, controller: ScrollController) -> Self {
        let theme = use_theme();
        let inner = RawScrollView::controlled(style, controller)
            .background(theme.surface)
            .corner_radius(theme.radius_medium)
            .scrollbar_color(with_alpha(theme.text_secondary, 80))
            .scrollbar_hover_color(with_alpha(theme.text_secondary, 170))
            .scrollbar_pressed_color(with_alpha(theme.text_primary, 220));
        ScrollView { inner }
    }

    pub fn child(mut self, widget: BoxedWidget) -> Self {
        self.inner = self.inner.child(widget);
        self
    }

    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.inner = self.inner.with_children(widgets);
        self
    }

    pub fn customize(mut self, customize: impl FnOnce(&mut RawScrollView)) -> Self {
        customize(&mut self.inner);
        self
    }

    /// Overrides the fill painted behind the scrollable content — otherwise
    /// always `theme.surface`, which isn't necessarily the color a caller
    /// wants directly behind this particular list (e.g. a message thread
    /// sitting on `surface_elevated` while a sidebar list sits on `surface`).
    /// Spacing between children stacked inside the scrollable area —
    /// otherwise always `0.0`, which reads as a single continuous list
    /// (fine for e.g. a sidebar's rows) but crowds anything meant to look
    /// like separate items, such as chat bubbles.
    pub fn content_gap(mut self, gap: f32) -> Self {
        self.inner = self.inner.content_gap(gap);
        self
    }
}

impl Widget for ScrollView {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        Widget::children(&mut self.inner)
    }
}

/// A virtualized list with a theme-colored surface and scrollbar.
pub struct VirtualList {
    inner: RawVirtualList,
}
impl_styled_inner!(VirtualList);

impl VirtualList {
    pub fn new(
        style: Style,
        state: VirtualListState,
        controller: ScrollController,
        viewport_height: f32,
        item: impl Fn(usize) -> BoxedWidget + 'static,
    ) -> Self {
        let theme = use_theme();
        let inner = RawVirtualList::new(style, state, controller, viewport_height, item)
            .background(theme.surface)
            .corner_radius(theme.radius_medium)
            .scrollbar_color(with_alpha(theme.text_secondary, 80))
            .scrollbar_hover_color(with_alpha(theme.text_secondary, 170))
            .scrollbar_pressed_color(with_alpha(theme.text_primary, 220));
        Self { inner }
    }

    pub fn overscan(mut self, rows: usize) -> Self {
        self.inner = self.inner.overscan(rows);
        self
    }

    pub fn scrollbar(mut self, visible: bool) -> Self {
        self.inner = self.inner.scrollbar(visible);
        self
    }
}

impl Widget for VirtualList {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }

    fn on_layout(&self, rect: Rect) -> bool {
        self.inner.on_layout(rect)
    }

    fn reports_layout(&self) -> bool {
        self.inner.reports_layout()
    }
}
