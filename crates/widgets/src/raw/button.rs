use super::*;

/// An unstyled clickable region. Its common style supplies the box paint;
/// combine it with [`RawText`] as a child for a labeled button.
pub struct RawButton {
    pub style: creamui_core::Style,
    pub children: Vec<BoxedWidget>,
    pub on_click: Rc<dyn Fn()>,
    pub on_click_at: Option<Rc<dyn Fn(Point)>>,
    pub on_drag: Option<Rc<dyn Fn(Point, Rect)>>,
    pub on_drag_start: Option<Rc<dyn Fn(Point, Rect)>>,
    pub on_drag_end: Option<Rc<dyn Fn()>>,
    pub disabled: bool,
}

impl RawButton {
    /// Builds a button from CreamUI's common style. A legacy
    /// `creamui_core::layout::Style` is accepted through `Into`.
    pub fn new(style: impl Into<creamui_core::Style>, on_click: impl Fn() + 'static) -> Self {
        let style = style.into();
        RawButton {
            style,
            children: Vec::new(),
            on_click: Rc::new(on_click),
            on_click_at: None,
            on_drag: None,
            on_drag_start: None,
            on_drag_end: None,
            disabled: false,
        }
    }

    /// Returns the complete style declaration consumed by this button.
    pub fn style_declaration(&self) -> &creamui_core::Style {
        &self.style
    }

    pub fn child(mut self, widget: BoxedWidget) -> Self {
        self.children.push(widget);
        self
    }

    pub fn with_click_position(mut self, on_click: impl Fn(Point) + 'static) -> Self {
        self.on_click_at = Some(Rc::new(on_click));
        self
    }

    pub fn with_drag(mut self, on_drag: impl Fn(Point, Rect) + 'static) -> Self {
        self.on_drag = Some(Rc::new(on_drag));
        self
    }

    pub fn with_drag_start(mut self, on_drag_start: impl Fn(Point, Rect) + 'static) -> Self {
        self.on_drag_start = Some(Rc::new(on_drag_start));
        self
    }

    pub fn with_drag_end(mut self, on_drag_end: impl Fn() + 'static) -> Self {
        self.on_drag_end = Some(Rc::new(on_drag_end));
        self
    }

    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.children = widgets;
        self
    }

    /// While `true`, the button reports no click handler (so it truly can't
    /// be activated, not just visually dimmed) and shows a "not allowed"
    /// cursor instead of the usual pointer.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Widget for RawButton {
    fn style_state(&self) -> creamui_core::StyleState {
        creamui_core::StyleState::NORMAL.with_disabled(self.disabled)
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        if self.disabled {
            return None;
        }
        let click = self.on_click.clone();
        Some(Rc::new(move |input| {
            if !input.modifiers.ctrl && matches!(input.key, Key::Enter | Key::Char(' ')) {
                click();
            }
        }))
    }
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        if self.disabled {
            None
        } else {
            Some(self.on_click.clone())
        }
    }

    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        if self.disabled {
            None
        } else {
            self.on_click_at.clone()
        }
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        (!self.disabled).then(|| self.on_drag.clone()).flatten()
    }

    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        (!self.disabled)
            .then(|| self.on_drag_start.clone())
            .flatten()
    }

    fn on_drag_end(&self) -> Option<Rc<dyn Fn()>> {
        (!self.disabled).then(|| self.on_drag_end.clone()).flatten()
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }
}
