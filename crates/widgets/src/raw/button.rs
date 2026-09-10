use super::*;

/// Visual properties that can change while a [`RawButton`] is hovered or
/// pressed. This deliberately contains only paint-time properties: changing
/// layout in response to a pointer state would make hit testing unstable.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ButtonVisualStyle {
    pub background: Option<Color>,
    pub border: Option<(Color, f32)>,
    pub corner_radius: Option<f32>,
}

impl ButtonVisualStyle {
    pub const fn new() -> Self {
        Self {
            background: None,
            border: None,
            corner_radius: None,
        }
    }

    pub const fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub const fn border(mut self, color: Color, width: f32) -> Self {
        self.border = Some((color, width));
        self
    }

    pub const fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = Some(radius);
        self
    }
}

/// Complete, reusable style declaration for a [`RawButton`].
///
/// `normal` establishes the resting paint; `hover` and `pressed` are patches
/// over it. The `layout` is intentionally separate and remains fixed across
/// interaction states, so the rendered bounds and hit target cannot shift
/// underneath the pointer.
#[derive(Clone, Debug, PartialEq)]
pub struct RawButtonStyle {
    pub layout: Style,
    pub normal: ButtonVisualStyle,
    pub hover: ButtonVisualStyle,
    pub pressed: ButtonVisualStyle,
    pub focus_color: Option<Color>,
}

impl RawButtonStyle {
    pub fn new(layout: Style) -> Self {
        Self {
            layout,
            normal: ButtonVisualStyle::default(),
            hover: ButtonVisualStyle::default(),
            pressed: ButtonVisualStyle::default(),
            focus_color: None,
        }
    }

    pub fn background(mut self, color: Color) -> Self {
        self.normal.background = Some(color);
        self
    }

    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.normal.border = Some((color, width));
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.normal.corner_radius = Some(radius);
        self
    }

    pub fn hover(mut self, style: ButtonVisualStyle) -> Self {
        self.hover = style;
        self
    }

    pub fn pressed(mut self, style: ButtonVisualStyle) -> Self {
        self.pressed = style;
        self
    }

    pub fn focus_color(mut self, color: Color) -> Self {
        self.focus_color = Some(color);
        self
    }
}

impl From<Style> for RawButtonStyle {
    fn from(layout: Style) -> Self {
        Self::new(layout)
    }
}

/// An unstyled clickable region. Paints only its `background`/`border` if
/// set; combine with [`RawText`] as a child for a labeled button.
pub struct RawButton {
    pub hover_background: Option<Color>,
    pub pressed_background: Option<Color>,
    /// Paint overrides applied while the pointer is over the button.
    pub hover_style: ButtonVisualStyle,
    /// Paint overrides applied while the primary pointer button is down.
    /// Unspecified fields inherit from `hover_style`, then the resting style.
    pub pressed_style: ButtonVisualStyle,
    pub focus_color: Option<Color>,
    pub style: Style,
    pub background: Option<Color>,
    pub corner_radius: f32,
    pub border: Option<(Color, f32)>,
    pub children: Vec<BoxedWidget>,
    pub on_click: Rc<dyn Fn()>,
    pub disabled: bool,
}

impl RawButton {
    /// Builds a button from a layout `Style` or from a reusable
    /// [`RawButtonStyle`].
    pub fn new(style: impl Into<RawButtonStyle>, on_click: impl Fn() + 'static) -> Self {
        let style = style.into();
        RawButton {
            hover_background: style.hover.background,
            pressed_background: style.pressed.background,
            hover_style: style.hover,
            pressed_style: style.pressed,
            focus_color: style.focus_color,
            style: style.layout,
            background: style.normal.background,
            corner_radius: style.normal.corner_radius.unwrap_or(0.0),
            border: style.normal.border,
            children: Vec::new(),
            on_click: Rc::new(on_click),
            disabled: false,
        }
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// Sets the full visual treatment used while hovered.
    pub fn hover_style(mut self, style: ButtonVisualStyle) -> Self {
        self.hover_style = style;
        self
    }

    /// Sets the full visual treatment used while pressed.
    pub fn pressed_style(mut self, style: ButtonVisualStyle) -> Self {
        self.pressed_style = style;
        self
    }

    /// Convenience equivalent to `hover_style(ButtonVisualStyle::new().background(color))`.
    pub fn hover_background(mut self, color: Color) -> Self {
        self.hover_background = Some(color);
        self
    }

    /// Convenience equivalent to `pressed_style(ButtonVisualStyle::new().background(color))`.
    pub fn pressed_background(mut self, color: Color) -> Self {
        self.pressed_background = Some(color);
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }
    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border = Some((color, width));
        self
    }

    pub fn layout_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Replaces the declarative base and interaction style in one operation.
    /// Individual shorthand methods such as [`Self::background`] can refine it
    /// afterwards.
    pub fn visual_style(mut self, style: RawButtonStyle) -> Self {
        self.style = style.layout;
        self.background = style.normal.background;
        self.border = style.normal.border;
        self.corner_radius = style.normal.corner_radius.unwrap_or(0.0);
        self.hover_background = style.hover.background;
        self.pressed_background = style.pressed.background;
        self.hover_style = style.hover;
        self.pressed_style = style.pressed;
        self.focus_color = style.focus_color;
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
    fn style(&self) -> Style {
        self.style.clone()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let interaction_style = if !self.disabled && painter.pressed(rect) {
            Some(self.pressed_style)
        } else if !self.disabled && painter.hovered(rect) {
            Some(self.hover_style)
        } else {
            None
        };
        let is_pressed = !self.disabled && painter.pressed(rect);
        let is_hovered = !self.disabled && painter.hovered(rect);
        let background = interaction_style
            .and_then(|style| style.background)
            .or(if is_pressed {
                self.pressed_background
            } else {
                None
            })
            .or(if is_pressed || is_hovered {
                self.hover_style.background
            } else {
                None
            })
            .or(if is_hovered || is_pressed {
                self.hover_background
            } else {
                None
            })
            .or(self.background);
        let border = interaction_style
            .and_then(|style| style.border)
            .or(if is_pressed || is_hovered {
                self.hover_style.border
            } else {
                None
            })
            .or(self.border);
        let corner_radius = interaction_style
            .and_then(|style| style.corner_radius)
            .or(if is_pressed || is_hovered {
                self.hover_style.corner_radius
            } else {
                None
            })
            .unwrap_or(self.corner_radius);
        if let Some(color) = background {
            painter.fill_rect(rect, color, corner_radius);
        }
        if let Some((color, width)) = border {
            painter.stroke_rect(rect, color, width, corner_radius);
        }
    }

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
