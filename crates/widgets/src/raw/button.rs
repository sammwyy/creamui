use super::*;

/// An unstyled clickable region. Its common style supplies the box paint;
/// combine it with [`RawText`] as a child for a labeled button.
pub struct RawButton {
    pub style: creamui_core::Style,
    pub children: Vec<BoxedWidget>,
    pub on_click: Rc<dyn Fn()>,
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
            disabled: false,
        }
    }

    pub fn background(mut self, color: Color) -> Self {
        self.style.paint.background = Some(color.into());
        self
    }

    /// Sets the full visual treatment used while hovered.
    pub fn hover_style(mut self, style: creamui_core::StateStyle) -> Self {
        self.style.states.hover = style;
        self
    }

    /// Sets the full visual treatment used while pressed.
    pub fn pressed_style(mut self, style: creamui_core::StateStyle) -> Self {
        self.style.states.pressed = style;
        self
    }

    /// Convenience equivalent to `hover_style(StateStyle::new().background(color))`.
    pub fn hover_background(mut self, color: Color) -> Self {
        self.style.states.hover.paint.background = Some(color.into());
        self
    }

    /// Convenience equivalent to `pressed_style(StateStyle::new().background(color))`.
    pub fn pressed_background(mut self, color: Color) -> Self {
        self.style.states.pressed.paint.background = Some(color.into());
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.style.paint.corner_radius = Some(radius);
        self
    }
    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.style.paint.border = Some(creamui_core::Border::new(color, width));
        self
    }

    pub fn layout_style(mut self, style: Style) -> Self {
        self.style.layout = style;
        self
    }

    /// Replaces the complete declaration.
    pub fn common_style(mut self, style: creamui_core::Style) -> Self {
        self.style = style;
        self
    }

    /// Returns the complete style declaration consumed by this button.
    pub fn style_declaration(&self) -> &creamui_core::Style {
        &self.style
    }

    pub fn focus_color(mut self, color: Color) -> Self {
        self.style.states.focus.paint.outline = Some(creamui_core::Border::new(color, 2.0));
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

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }
}
