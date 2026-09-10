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
    pub style: creamui_core::Style,
    pub children: Vec<BoxedWidget>,
}

impl RawTabs {
    pub fn new(style: impl Into<creamui_core::Style>) -> Self {
        RawTabs {
            style: style.into(),
            children: Vec::new(),
        }
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
        let mut style = self.style.clone();
        style.layout = Style {
            display: creamui_core::layout::Display::Flex,
            flex_direction: creamui_core::layout::FlexDirection::Row,
            ..style.layout
        };
        style
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }
}

/// An unstyled vertical nav rail: always lays out [`RawTab`] children
/// top-to-bottom. Same shape as [`RawTabs`] — owns only layout and
/// background — but for the "sidebar switches the visible view" pattern
/// instead of a horizontal tab bar.
pub struct RawSidebar {
    pub style: creamui_core::Style,
    pub children: Vec<BoxedWidget>,
}

impl RawSidebar {
    pub fn new(style: impl Into<creamui_core::Style>) -> Self {
        RawSidebar {
            style: style.into(),
            children: Vec::new(),
        }
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
        let mut style = self.style.clone();
        style.layout = Style {
            display: creamui_core::layout::Display::Flex,
            flex_direction: creamui_core::layout::FlexDirection::Column,
            ..style.layout
        };
        style
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

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
/// Its common style supplies the box paint; while `active`, the widget adds a
/// solid indicator bar along one edge. Everything else comes from children.
pub struct RawTab {
    pub style: creamui_core::Style,
    pub active: bool,
    pub indicator: Option<(TabIndicatorSide, Color, f32)>,
    pub children: Vec<BoxedWidget>,
    pub on_click: Rc<dyn Fn()>,
    pub on_hover: Option<Rc<dyn Fn(bool)>>,
    pub disabled: bool,
}

impl RawTab {
    pub fn new(
        style: impl Into<creamui_core::Style>,
        active: bool,
        on_click: impl Fn() + 'static,
    ) -> Self {
        RawTab {
            style: style.into(),
            active,
            indicator: None,
            children: Vec::new(),
            on_click: Rc::new(on_click),
            on_hover: None,
            disabled: false,
        }
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

    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn style_state(&self) -> creamui_core::StyleState {
        creamui_core::StyleState::NORMAL.with_disabled(self.disabled)
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
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
