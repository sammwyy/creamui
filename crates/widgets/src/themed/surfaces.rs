use super::*;
use crate::layout::StyleExt;
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
            popup: elevation::overlay_fill(&theme),
            active: elevation::with_alpha(theme.accent, 46),
            border: elevation::overlay_border(&theme),
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
    pub fn new(colors: MenuColors, mut style: Style) -> Self {
        if style.padding == Style::default().padding {
            style = style.padding_all(4.0);
        }
        let theme = use_theme();
        let mut inner = RawView::new(style)
            .background(colors.popup)
            .border(colors.border, 1.0)
            .corner_radius(colors.popup_radius);
        if !theme.high_contrast {
            inner = inner.box_shadow(elevation::Elevation::Overlay.shadow(&theme));
        }
        Self { inner }
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
        mut style: Style,
        label: impl Into<String>,
        active: bool,
        on_click: impl Fn() + 'static,
    ) -> Self {
        let color = if active {
            colors.active
        } else {
            elevation::with_alpha(colors.popup, 0)
        };
        if style.padding == Style::default().padding {
            style = style.padding_xy(12.0, 8.0);
        }
        let text = RawText::new(label, colors.text, 13.0).text_align(TextAlign::Start);
        Self {
            inner: RawButton::new(style, on_click)
                .background(color)
                .corner_radius(colors.item_radius)
                .hover_style(creamui_core::StateStyle::new().background(colors.active))
                .focus_style(creamui_core::StateStyle::new().outline(colors.border, 2.0))
                .child(Box::new(text)),
        }
    }
}

impl Widget for MenuItem {
    fn focusable(&self) -> bool {
        self.inner.focusable()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }
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
    pub fn new(mut style: Style) -> Self {
        let theme = use_theme();
        if style.padding == Style::default().padding {
            style = style.padding_all(theme.spacing_large);
        }
        let mut inner = RawView::new(style)
            .background(elevation::group_fill(&theme))
            .border(theme.border, 1.0)
            .corner_radius(theme.card_radius);
        if !theme.high_contrast {
            inner = inner.box_shadow(elevation::Elevation::Raised.shadow(&theme));
        }
        Card { inner }
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

    fn children(&mut self) -> Vec<BoxedWidget> {
        Widget::children(&mut self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::StyleExt;

    #[test]
    fn themed_surfaces_have_distinct_fill_and_inset_content() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let theme = use_theme();
            let colors = MenuColors::dark();
            let popup = MenuPopup::new(colors, Style::default());
            assert_eq!(popup.style().paint.background, Some(colors.popup.into()));
            assert!(popup.style().paint.box_shadow.is_some());
            assert!(popup.style().layout.padding.left != LengthPercentage::Length(0.0));
            let card = Card::new(Style::default());
            assert!(card.style().layout.padding.left != LengthPercentage::Length(0.0));
            assert_eq!(card.style().paint.corner_radius, Some(theme.card_radius));
            assert!(card.style().paint.box_shadow.is_some());

            let custom = Card::new(Style::default().padding_all(24.0));
            assert_eq!(
                custom.style().layout.padding.left,
                LengthPercentage::Length(24.0)
            );
        });
    }

    #[test]
    fn menu_item_does_not_apply_container_layout_twice() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let colors = MenuColors::dark();
            let mut item = MenuItem::new(colors, Style::default(), "Open", false, || {});
            assert!(item.focusable());
            assert!(item.style().layout.padding.left != LengthPercentage::Length(0.0));
            assert_eq!(
                item.children()[0].style().layout.padding.left,
                LengthPercentage::Length(0.0)
            );
        });
    }
}
