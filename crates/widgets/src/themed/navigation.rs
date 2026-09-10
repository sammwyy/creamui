use super::*;
/// Shared visual tokens for [`Tabs`]/[`Tab`] and [`Sidebar`]/[`SidebarItem`],
/// the same pattern as [`MenuColors`]: derive from a theme, override
/// individual colors if needed.
#[derive(Clone, Copy)]
pub struct TabColors {
    /// Canvas behind the whole group, visible in the gaps between entries.
    pub background: creamui_theme::Color,
    /// Optional background for inactive filled tabs. Set this to `None` for
    /// text-only or indicator tabs.
    pub inactive_background: Option<creamui_theme::Color>,
    pub active_background: creamui_theme::Color,
    pub hover_background: creamui_theme::Color,
    pub indicator: creamui_theme::Color,
    pub text: creamui_theme::Color,
    pub active_text: creamui_theme::Color,
    pub muted_text: creamui_theme::Color,
    pub radius: f32,
    pub container_radius: f32,
    pub selection: SelectionStyle,
    pub indicator_thickness: f32,
    pub gap: f32,
    pub icon_size: f32,
    pub icon_radius: f32,
    pub item_gap: f32,
    pub separator: creamui_theme::Color,
}

impl TabColors {
    /// Colours and geometry for a horizontal tab group.
    pub fn dark() -> Self {
        let theme = use_theme();
        Self {
            background: theme.surface,
            inactive_background: Some(theme.surface_hover),
            active_background: theme.accent,
            hover_background: theme.surface_elevated,
            indicator: theme.accent,
            text: theme.text_primary,
            active_text: Color::rgb(0x33, 0x2e, 0x34),
            muted_text: theme.text_secondary,
            radius: theme.tab_radius,
            container_radius: theme.tabs_radius,
            selection: theme.tab_selection,
            indicator_thickness: theme.indicator_thickness,
            gap: theme.tab_gap,
            icon_size: 0.0,
            icon_radius: 0.0,
            item_gap: 0.0,
            separator: theme.border,
        }
    }

    /// Colours and geometry for a vertical sidebar. Kept separate because a
    /// theme may intentionally give navigation a different silhouette.
    pub fn sidebar() -> Self {
        let theme = use_theme();
        Self {
            // Navigation remains integrated with the app canvas; the
            // encapsulated content card is the elevated material.
            background: theme.surface,
            inactive_background: None,
            active_background: theme.accent,
            hover_background: theme.surface_elevated,
            indicator: theme.accent,
            text: theme.text_primary,
            active_text: Color::rgb(0x33, 0x2e, 0x34),
            muted_text: theme.text_secondary,
            radius: theme.sidebar_item_radius,
            container_radius: theme.sidebar_radius,
            selection: theme.sidebar_selection,
            indicator_thickness: theme.indicator_thickness,
            gap: theme.sidebar_gap,
            icon_size: theme.sidebar_icon_size,
            icon_radius: theme.sidebar_icon_radius,
            item_gap: theme.sidebar_item_gap,
            separator: theme.border,
        }
    }
}

/// A themed horizontal tab bar. Like [`MenuBar`], it owns only layout and
/// paint; applications compose [`Tab`] children and keep the selected index
/// in their own `Signal`. For a vertical rail, use [`Sidebar`] instead.
pub struct Tabs {
    inner: RawTabs,
}
impl_styled_inner!(Tabs);

impl Tabs {
    /// The supplied [`Style`] is preserved exactly, including `gap`, width,
    /// margins, alignment, and padding.
    pub fn new(colors: TabColors, style: Style) -> Self {
        Self {
            inner: RawTabs::new(style)
                .background(colors.background)
                .corner_radius(colors.container_radius),
        }
    }

    /// Replaces the container layout style after construction.
    /// Sets horizontal and vertical space between tab entries.
    pub fn gap(mut self, gap: f32) -> Self {
        self.inner.style.gap = creamui_core::layout::Size {
            width: LengthPercentage::Length(gap),
            height: LengthPercentage::Length(gap),
        };
        self
    }

    pub fn child(mut self, child: BoxedWidget) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.inner = self.inner.with_children(widgets);
        self
    }
}

impl Widget for Tabs {
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

/// A controlled, themed tab entry for [`Tabs`]: an accent indicator bar along
/// the bottom edge while `active`, muted text otherwise. `active` is supplied
/// by the app so a tab bar can be rebuilt reactively with no hidden widget
/// state — the same pattern as [`MenuItem`].
pub struct Tab {
    inner: RawTab,
}
impl_styled_inner!(Tab);

/// How a group of tabs receives horizontal space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabSizing {
    /// Each tab is only as wide as its label and padding.
    #[default]
    Content,
    /// Every tab is normalized to the group's widest label.
    Equal,
    /// Tabs divide the full width of their container.
    Fill,
}

/// Produces one layout style per tab label for the chosen width policy.
pub fn tab_styles(
    labels: &[&str],
    sizing: TabSizing,
    height: f32,
    horizontal_padding: f32,
) -> Vec<Style> {
    let equal_width = labels
        .iter()
        .map(|label| {
            crate::text_metrics::measure(label, 14.0, crate::text_metrics::unbounded_width()).0
        })
        .fold(0.0_f32, f32::max)
        + horizontal_padding * 2.0;

    labels
        .iter()
        .map(|_| {
            let mut style = centered_box_style(horizontal_padding);
            style.size.height = Dimension::Length(height);
            match sizing {
                TabSizing::Content => {}
                TabSizing::Equal => style.size.width = Dimension::Length(equal_width),
                TabSizing::Fill => {
                    style.flex_grow = 1.0;
                    style.flex_basis = Dimension::Length(0.0);
                }
            }
            style
        })
        .collect()
}

impl Tab {
    pub fn new(
        colors: TabColors,
        style: Style,
        label: impl Into<String>,
        active: bool,
        on_click: impl Fn() + 'static,
    ) -> Self {
        let text_color = if active && colors.selection == SelectionStyle::Filled {
            colors.active_text
        } else if active {
            colors.text
        } else {
            colors.muted_text
        };
        let text = RawText::new(label, text_color, 14.0)
            .text_align(TextAlign::Center)
            .layout(style.clone());
        let mut inner = RawTab::new(style, active, on_click)
            .corner_radius(colors.radius)
            .child(Box::new(text));
        match colors.selection {
            SelectionStyle::Filled => {
                if let Some(background) = if active {
                    Some(colors.active_background)
                } else {
                    colors.inactive_background
                } {
                    inner = inner.background(background);
                }
            }
            SelectionStyle::Indicator => {
                inner = inner.indicator(
                    TabIndicatorSide::Bottom,
                    colors.indicator,
                    colors.indicator_thickness,
                );
            }
        }
        if !active {
            inner.style.states.hover.paint.background = Some(colors.hover_background.into());
            inner.style.states.pressed.paint.background = Some(colors.hover_background.into());
        }
        inner.style.states.focus.paint.outline =
            Some(creamui_core::Border::new(colors.indicator, 2.0));
        Self { inner }
    }
}

impl Widget for Tab {
    fn focusable(&self) -> bool {
        self.inner.focusable()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }
    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, caret: bool) {
        self.inner.paint_focused_overlay(painter, rect, caret)
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

/// A themed vertical nav rail. Same shape as [`Tabs`], but for the "sidebar
/// switches the visible view" pattern: applications compose [`SidebarItem`]
/// children and keep the selected index in their own `Signal`.
pub struct Sidebar {
    inner: RawSidebar,
}
impl_styled_inner!(Sidebar);

impl Sidebar {
    pub fn new(colors: TabColors, style: Style) -> Self {
        Self {
            inner: RawSidebar::new(style)
                .background(colors.background)
                .corner_radius(colors.container_radius),
        }
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.inner.style.gap = creamui_core::layout::Size {
            width: LengthPercentage::Length(gap),
            height: LengthPercentage::Length(gap),
        };
        self
    }

    pub fn child(mut self, child: BoxedWidget) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.inner = self.inner.with_children(widgets);
        self
    }
}

impl Widget for Sidebar {
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

/// A controlled, themed entry for [`Sidebar`]: an accent indicator bar along
/// the left edge while `active`, muted text otherwise — the vertical
/// counterpart of [`Tab`].
pub struct SidebarItem {
    inner: RawTab,
}
impl_styled_inner!(SidebarItem);

/// A divider for groups inside a [`Sidebar`]. Its label is optional; when
/// supplied it becomes the small, muted section title used by settings apps.
pub struct SidebarSeparator {
    colors: TabColors,
    style: Style,
    label: Option<String>,
}

impl SidebarSeparator {
    pub fn new(colors: TabColors, style: Style) -> Self {
        Self {
            colors,
            style,
            label: None,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Widget for SidebarSeparator {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(label) = &self.label {
            painter.fill_text(rect, label, self.colors.muted_text, 11.0, TextAlign::Start);
        } else {
            let y = rect.y + rect.height / 2.0;
            painter.fill_rect(
                Rect {
                    x: rect.x,
                    y,
                    width: rect.width,
                    height: 1.0,
                },
                self.colors.separator,
                0.0,
            );
        }
    }
}

impl SidebarItem {
    pub fn new(
        colors: TabColors,
        style: Style,
        label: impl Into<String>,
        active: bool,
        on_click: impl Fn() + 'static,
    ) -> Self {
        Self::build(
            colors,
            style,
            label.into(),
            None,
            active,
            false,
            None,
            on_click,
        )
    }

    /// Adds a small rounded square icon before the item label.
    pub fn with_icon(
        colors: TabColors,
        style: Style,
        label: impl Into<String>,
        icon_color: creamui_theme::Color,
        active: bool,
        on_click: impl Fn() + 'static,
    ) -> Self {
        Self::build(
            colors,
            style,
            label.into(),
            Some(icon_color),
            active,
            false,
            None,
            on_click,
        )
    }

    /// A sidebar item with a real pointer-hover state. Keep `hovered` in a
    /// [`creamui_reactive::Signal`] and pass its setter here; the renderer
    /// calls it on pointer entry/exit and the item is rebuilt with the soft
    /// hover background on the next reactive frame.
    pub fn with_hover(
        colors: TabColors,
        style: Style,
        label: impl Into<String>,
        active: bool,
        hovered: bool,
        on_hover: impl Fn(bool) + 'static,
        on_click: impl Fn() + 'static,
    ) -> Self {
        Self::build(
            colors,
            style,
            label.into(),
            None,
            active,
            hovered,
            Some(Rc::new(on_hover)),
            on_click,
        )
    }

    /// Combines a coloured icon with the reactive hover state.
    pub fn with_icon_hover(
        colors: TabColors,
        style: Style,
        label: impl Into<String>,
        icon_color: creamui_theme::Color,
        active: bool,
        hovered: bool,
        on_hover: impl Fn(bool) + 'static,
        on_click: impl Fn() + 'static,
    ) -> Self {
        Self::build(
            colors,
            style,
            label.into(),
            Some(icon_color),
            active,
            hovered,
            Some(Rc::new(on_hover)),
            on_click,
        )
    }

    fn build(
        colors: TabColors,
        style: Style,
        label: String,
        icon_color: Option<creamui_theme::Color>,
        active: bool,
        hovered: bool,
        on_hover: Option<Rc<dyn Fn(bool)>>,
        on_click: impl Fn() + 'static,
    ) -> Self {
        let text_color = if active && colors.selection == SelectionStyle::Filled {
            colors.active_text
        } else if active {
            colors.text
        } else {
            colors.muted_text
        };
        let text = RawText::new(label, text_color, 14.0).text_align(TextAlign::Start);
        let content: BoxedWidget = if let Some(icon_color) = icon_color {
            let content_style = Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Percent(1.0),
                    height: creamui_core::layout::Dimension::Percent(1.0),
                },
                display: creamui_core::layout::Display::Flex,
                flex_direction: creamui_core::layout::FlexDirection::Row,
                align_items: Some(AlignItems::Center),
                gap: creamui_core::layout::Size {
                    width: LengthPercentage::Length(colors.item_gap),
                    height: LengthPercentage::Length(colors.item_gap),
                },
                ..Default::default()
            };
            let icon_style = Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(colors.icon_size),
                    height: creamui_core::layout::Dimension::Length(colors.icon_size),
                },
                flex_shrink: 0.0,
                ..Default::default()
            };
            let text_style = Style {
                flex_grow: 1.0,
                ..Default::default()
            };
            Box::new(
                RawView::new(content_style)
                    .child(Box::new(
                        RawView::new(icon_style)
                            .background(icon_color)
                            .corner_radius(colors.icon_radius),
                    ))
                    .child(Box::new(text.layout(text_style))),
            )
        } else {
            Box::new(text.layout(style.clone()))
        };
        let mut inner = RawTab::new(style, active, on_click)
            .corner_radius(colors.radius)
            .child(content);
        match colors.selection {
            SelectionStyle::Filled => {
                if active {
                    inner = inner.background(colors.active_background);
                } else if hovered {
                    inner = inner.background(colors.hover_background);
                } else if let Some(background) = colors.inactive_background {
                    inner = inner.background(background);
                }
            }
            SelectionStyle::Indicator => {
                inner = inner.indicator(
                    TabIndicatorSide::Left,
                    colors.indicator,
                    colors.indicator_thickness,
                );
            }
        }
        if !active {
            inner.style.states.hover.paint.background = Some(colors.hover_background.into());
            inner.style.states.pressed.paint.background = Some(colors.hover_background.into());
        }
        inner.style.states.focus.paint.outline =
            Some(creamui_core::Border::new(colors.indicator, 2.0));
        inner.on_hover = on_hover;
        Self { inner }
    }
}

impl Widget for SidebarItem {
    fn focusable(&self) -> bool {
        self.inner.focusable()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }
    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, caret: bool) {
        self.inner.paint_focused_overlay(painter, rect, caret)
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
    fn on_hover(&self) -> Option<Rc<dyn Fn(bool)>> {
        self.inner.on_hover.clone()
    }
}
