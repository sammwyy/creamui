use super::*;
/// A themed surface with the standard card background and rounded corners.
pub struct Card {
    inner: RawView,
}
impl_styled_inner!(Card);

/// Shared visual tokens for application menu bars and popovers. Apps can
/// derive these from a theme and override individual colors without copying
/// menu geometry throughout their UI.
#[derive(Clone, Copy)]
pub struct MenuColors {
    pub bar: creamui_theme::Color,
    pub popup: creamui_theme::Color,
    pub active: creamui_theme::Color,
    pub border: creamui_theme::Color,
    pub text: creamui_theme::Color,
    pub muted_text: creamui_theme::Color,
    pub popup_radius: f32,
    pub item_radius: f32,
}

impl MenuColors {
    pub fn dark() -> Self {
        let theme = use_theme();
        Self {
            bar: theme.surface,
            popup: theme.surface_hover,
            active: theme.border,
            border: theme.border_strong,
            text: theme.text_primary,
            muted_text: theme.text_secondary,
            popup_radius: theme.menu_radius,
            item_radius: theme.menu_item_radius,
        }
    }
}

/// A reusable application menu-bar surface. It owns only layout and paint;
/// applications compose [`MenuItem`] children and retain their own menu state.
pub struct MenuBar {
    inner: RawView,
}
impl_styled_inner!(MenuBar);

impl MenuBar {
    pub fn new(colors: MenuColors, style: Style) -> Self {
        Self {
            inner: RawView::new(style).background(colors.bar),
        }
    }
    pub fn child(mut self, child: BoxedWidget) -> Self {
        self.inner = self.inner.child(child);
        self
    }
}

impl Widget for MenuBar {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect)
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
}

/// A compact menu popover surface. Give it an absolute-positioned `Style`
/// when it should float over application content.
pub struct MenuPopup {
    inner: RawView,
}
impl_styled_inner!(MenuPopup);

impl MenuPopup {
    pub fn new(colors: MenuColors, style: Style) -> Self {
        Self {
            inner: RawView::new(style)
                .background(colors.border)
                .corner_radius(colors.popup_radius),
        }
    }
    pub fn child(mut self, child: BoxedWidget) -> Self {
        self.inner = self.inner.child(child);
        self
    }
}

impl Widget for MenuPopup {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect)
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
}

/// A controlled clickable menu entry. `active` is supplied by the app so a
/// menu can be rebuilt reactively without hidden widget state.
pub struct MenuItem {
    inner: RawButton,
}
impl_styled_inner!(MenuItem);

impl MenuItem {
    pub fn new(
        colors: MenuColors,
        style: Style,
        label: impl Into<String>,
        active: bool,
        on_click: impl Fn() + 'static,
    ) -> Self {
        let color = if active { colors.active } else { colors.popup };
        let text = RawText::new(
            label,
            if active {
                colors.text
            } else {
                colors.muted_text
            },
            13.0,
        )
        .text_align(TextAlign::Start)
        .layout(style.clone());
        Self {
            inner: RawButton::new(style, on_click)
                .background(color)
                .corner_radius(colors.item_radius)
                .child(Box::new(text)),
        }
    }
}

impl Widget for MenuItem {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect)
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }
}

impl Card {
    pub fn new(style: Style) -> Self {
        let theme = use_theme();
        Card {
            inner: RawView::new(style)
                .background(theme.surface_elevated)
                .corner_radius(theme.card_radius),
        }
    }

    pub fn child(mut self, widget: BoxedWidget) -> Self {
        self.inner = self.inner.child(widget);
        self
    }

    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.inner = self.inner.with_children(widgets);
        self
    }
}

impl Widget for Card {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn paint_fingerprint(&self) -> Option<u64> {
        self.inner.paint_fingerprint()
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        Widget::children(&mut self.inner)
    }
}

#[cfg(test)]
mod card_fingerprint_tests {
    use super::*;

    #[test]
    fn delegates_to_the_inner_view_instead_of_the_default_none() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(
                creamui_theme::Theme::dark(),
            ));
            let a = Card::new(Style::default());
            let b = Card::new(Style::default());
            assert!(a.paint_fingerprint().is_some());
            assert_eq!(a.paint_fingerprint(), b.paint_fingerprint());
        });
    }
}
