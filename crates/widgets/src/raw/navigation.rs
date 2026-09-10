use super::*;
/// Which edge of a [`RawTab`] its active indicator bar is drawn on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabIndicatorSide {
    Left,
    Right,
    Top,
    Bottom,
}

/// An unstyled horizontal tab bar: always lays out [`RawTab`] children
/// left-to-right. Like every other raw widget it owns only layout and
/// background — which tab is active and what a click does live in the
/// [`RawTab`] children and whatever `Signal` the caller wires them to. For a
/// vertical stack of nav entries, use [`RawSidebar`] instead.
pub struct RawTabs {
    pub style: Style,
    pub background: Option<Color>,
    pub corner_radius: f32,
    pub children: Vec<BoxedWidget>,
}

impl RawTabs {
    pub fn new(style: Style) -> Self {
        RawTabs {
            style,
            background: None,
            corner_radius: 0.0,
            children: Vec::new(),
        }
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn layout_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn child(mut self, widget: BoxedWidget) -> Self {
        self.children.push(widget);
        self
    }

    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.children = widgets;
        self
    }
}

impl Widget for RawTabs {
    fn style(&self) -> creamui_core::Style {
        Style {
            display: creamui_core::layout::Display::Flex,
            flex_direction: creamui_core::layout::FlexDirection::Row,
            ..self.style.clone()
        }
        .into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(color) = self.background {
            painter.fill_rect(rect, color, self.corner_radius);
        }
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }
}

/// An unstyled vertical nav rail: always lays out [`RawTab`] children
/// top-to-bottom. Same shape as [`RawTabs`] — owns only layout and
/// background — but for the "sidebar switches the visible view" pattern
/// instead of a horizontal tab bar.
pub struct RawSidebar {
    pub style: Style,
    pub background: Option<Color>,
    pub corner_radius: f32,
    pub children: Vec<BoxedWidget>,
}

impl RawSidebar {
    pub fn new(style: Style) -> Self {
        RawSidebar {
            style,
            background: None,
            corner_radius: 0.0,
            children: Vec::new(),
        }
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn layout_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn child(mut self, widget: BoxedWidget) -> Self {
        self.children.push(widget);
        self
    }

    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.children = widgets;
        self
    }
}

impl Widget for RawSidebar {
    fn style(&self) -> creamui_core::Style {
        Style {
            display: creamui_core::layout::Display::Flex,
            flex_direction: creamui_core::layout::FlexDirection::Column,
            ..self.style.clone()
        }
        .into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(color) = self.background {
            painter.fill_rect(rect, color, self.corner_radius);
        }
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }
}

/// An unstyled, controlled tab button, shared by [`RawTabs`] and
/// [`RawSidebar`]. `active` is supplied by the caller —
/// typically compared against a `Signal<usize>` holding the selected tab
/// index — so a tab list can be rebuilt reactively with no hidden widget
/// state, the same pattern as [`RawCheckbox`].
///
/// Paints only its `background` and, while `active`, a solid indicator bar
/// along one edge; everything else (label, icon, padding) comes from its
/// children, so a fully custom tab look needs no more than picking colors.
pub struct RawTab {
    pub style: Style,
    pub active: bool,
    pub background: Option<Color>,
    pub hover_background: Option<Color>,
    pub pressed_background: Option<Color>,
    pub focus_color: Option<Color>,
    pub corner_radius: f32,
    pub indicator: Option<(TabIndicatorSide, Color, f32)>,
    pub children: Vec<BoxedWidget>,
    pub on_click: Rc<dyn Fn()>,
    pub on_hover: Option<Rc<dyn Fn(bool)>>,
    pub disabled: bool,
}

impl RawTab {
    pub fn new(style: Style, active: bool, on_click: impl Fn() + 'static) -> Self {
        RawTab {
            style,
            active,
            background: None,
            hover_background: None,
            pressed_background: None,
            focus_color: None,
            corner_radius: 0.0,
            indicator: None,
            children: Vec::new(),
            on_click: Rc::new(on_click),
            on_hover: None,
            disabled: false,
        }
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn hover_background(mut self, color: Color) -> Self {
        self.hover_background = Some(color);
        self
    }

    pub fn pressed_background(mut self, color: Color) -> Self {
        self.pressed_background = Some(color);
        self
    }

    pub fn focus_color(mut self, color: Color) -> Self {
        self.focus_color = Some(color);
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn layout_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Draws a solid `thickness`-px bar along `side` while `active` is true.
    pub fn indicator(mut self, side: TabIndicatorSide, color: Color, thickness: f32) -> Self {
        self.indicator = Some((side, color, thickness));
        self
    }

    /// Registers a pointer enter/leave callback. The renderer invokes it
    /// with `true` on entry and `false` on exit.
    pub fn on_hover(mut self, callback: impl Fn(bool) + 'static) -> Self {
        self.on_hover = Some(Rc::new(callback));
        self
    }

    pub fn child(mut self, widget: BoxedWidget) -> Self {
        self.children.push(widget);
        self
    }

    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.children = widgets;
        self
    }
}

impl Widget for RawTab {
    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        if self.disabled {
            None
        } else {
            Some(activate_on_key(self.on_click.clone()))
        }
    }

    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, _: bool) {
        if let Some(color) = self.focus_color {
            painter.stroke_rect(
                Rect {
                    x: rect.x - 2.,
                    y: rect.y - 2.,
                    width: rect.width + 4.,
                    height: rect.height + 4.,
                },
                color,
                2.,
                self.corner_radius + 2.,
            );
        }
    }

    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let background = if !self.disabled && painter.pressed(rect) {
            self.pressed_background
                .or(self.hover_background)
                .or(self.background)
        } else if !self.disabled && painter.hovered(rect) {
            self.hover_background.or(self.background)
        } else {
            self.background
        };
        if let Some(color) = background {
            painter.fill_rect(rect, color, self.corner_radius);
        }
        if self.active {
            if let Some((side, color, thickness)) = self.indicator {
                // Capsule shape, inset from the tab's own edges.
                let inset = (thickness * 1.5).min(rect.width.min(rect.height) * 0.25);
                let bar = match side {
                    TabIndicatorSide::Left => Rect {
                        x: rect.x,
                        y: rect.y + inset,
                        width: thickness,
                        height: (rect.height - inset * 2.0).max(0.0),
                    },
                    TabIndicatorSide::Right => Rect {
                        x: rect.x + rect.width - thickness,
                        y: rect.y + inset,
                        width: thickness,
                        height: (rect.height - inset * 2.0).max(0.0),
                    },
                    TabIndicatorSide::Top => Rect {
                        x: rect.x + inset,
                        y: rect.y,
                        width: (rect.width - inset * 2.0).max(0.0),
                        height: thickness,
                    },
                    TabIndicatorSide::Bottom => Rect {
                        x: rect.x + inset,
                        y: rect.y + rect.height - thickness,
                        width: (rect.width - inset * 2.0).max(0.0),
                        height: thickness,
                    },
                };
                painter.fill_rect(bar, color, thickness / 2.0);
            }
        }
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        (!self.disabled).then(|| self.on_click.clone())
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }

    fn on_hover(&self) -> Option<Rc<dyn Fn(bool)>> {
        self.on_hover.clone()
    }
}
