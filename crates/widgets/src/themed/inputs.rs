use super::*;
/// A themed single-line text input.
pub struct TextInput {
    inner: RawTextInput,
}
impl_styled_inner!(TextInput);

impl TextInput {
    /// The style used when none is given explicitly: a fixed 200x36 box,
    /// matching this widget's original hardcoded layout.
    pub fn default_style() -> Style {
        Style {
            size: creamui_core::layout::Size {
                width: creamui_core::layout::Dimension::Length(200.0),
                height: creamui_core::layout::Dimension::Length(36.0),
            },
            ..Default::default()
        }
    }

    pub fn new(value: impl Into<String>, on_change: impl Fn(String) + 'static) -> Self {
        Self::from_layout(Self::default_style(), value, on_change)
    }

    /// Same as [`TextInput::new`], but with full control over layout
    /// instead of the fixed 200x36 default.
    fn from_layout(
        style: Style,
        value: impl Into<String>,
        on_change: impl Fn(String) + 'static,
    ) -> Self {
        let theme = use_theme();
        let inner = RawTextInput::new(style, value, 14.0, theme.text_primary, on_change)
            .font_family(theme.font_family)
            .background(theme.surface_elevated)
            .border(theme.border, theme.input_border_width)
            .corner_radius(theme.input_radius)
            .selection_background(theme.selection_background)
            .selection_text_color(theme.selection_text);
        TextInput { inner }
    }

    /// Grayed-out text shown when the value is empty.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.placeholder(text, theme.text_disabled);
        self
    }

    /// Called on Enter, e.g. to submit a chat message or search field.
    pub fn on_submit(mut self, on_submit: impl Fn() + 'static) -> Self {
        self.inner = self.inner.on_submit(on_submit);
        self
    }

    pub fn clipboard_enabled(mut self, enabled: bool) -> Self {
        self.inner = self.inner.clipboard_enabled(enabled);
        self
    }

    /// A `TextInput` whose value is read from and written back to a
    /// [`crate::TextController`], instead of a manually wired `value` +
    /// `on_change` pair. The controller must be a handle the app keeps
    /// alive across renders (created once, e.g. in `main`, the same way a
    /// `Signal` is) — cloning it here is cheap and shares the same
    /// underlying state.
    pub fn controlled(controller: &crate::TextController) -> Self {
        Self::controlled_with_style(Self::default_style(), controller)
    }

    /// Same as [`TextInput::controlled`], but with full control over layout.
    pub fn controlled_with_style(style: Style, controller: &crate::TextController) -> Self {
        let set = controller.clone();
        Self::from_layout(style, controller.value(), move |next| set.set_value(next))
            .cursor(controller.cursor(), {
                let set = controller.clone();
                move |cursor| set.set_cursor(cursor)
            })
            .selection(controller.selection(), {
                let set = controller.clone();
                move |selection| set.set_selection(selection)
            })
    }

    pub fn cursor(mut self, cursor: usize, on_change: impl Fn(usize) + 'static) -> Self {
        self.inner = self.inner.cursor(cursor, on_change);
        self
    }
    pub fn selection(
        mut self,
        selection: crate::raw::TextSelection,
        on_change: impl Fn(crate::raw::TextSelection) + 'static,
    ) -> Self {
        self.inner = self.inner.selection(selection, on_change);
        self
    }
}

impl Widget for TextInput {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn focusable(&self) -> bool {
        self.inner.focusable()
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.inner.on_drag()
    }

    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.inner.on_drag_start()
    }

    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, caret_visible: bool) {
        if let Some(color) = self.inner.selection_background {
            painter.stroke_rect(
                rect,
                color,
                2.,
                self.inner.style.paint.corner_radius.unwrap_or(0.0),
            );
        }
        self.inner
            .paint_focused_overlay(painter, rect, caret_visible);
    }
}

/// A themed multi-line text editor. It shares `TextInput`'s controlled-value
/// API while choosing an editor-friendly 14px inset and surface treatment.
pub struct TextArea {
    inner: RawTextArea,
}
impl_styled_inner!(TextArea);

impl TextArea {
    pub fn default_style() -> Style {
        Style {
            size: creamui_core::layout::Size {
                width: creamui_core::layout::Dimension::Length(400.0),
                height: creamui_core::layout::Dimension::Length(240.0),
            },
            ..Default::default()
        }
    }
    pub fn new(value: impl Into<String>, on_change: impl Fn(String) + 'static) -> Self {
        Self::from_layout(Self::default_style(), value, on_change)
    }
    fn from_layout(
        style: Style,
        value: impl Into<String>,
        on_change: impl Fn(String) + 'static,
    ) -> Self {
        let theme = use_theme();
        Self {
            inner: RawTextArea::new(style, value, 14.0, theme.text_primary, on_change)
                .font_family(theme.font_family)
                .background(theme.surface_elevated)
                .border(theme.border, theme.input_border_width)
                .corner_radius(theme.textarea_radius)
                .selection_background(theme.selection_background)
                .selection_text_color(theme.selection_text),
        }
    }
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.placeholder(text, theme.text_disabled);
        self
    }

    pub fn alternating_line_background(mut self, color: creamui_theme::Color) -> Self {
        self.inner = self.inner.alternating_line_background(color);
        self
    }

    pub fn active_line_background(mut self, color: creamui_theme::Color) -> Self {
        self.inner = self.inner.active_line_background(color);
        self
    }

    pub fn cursor(mut self, cursor: usize, on_change: impl Fn(usize) + 'static) -> Self {
        self.inner = self.inner.cursor(cursor, on_change);
        self
    }

    pub fn selection(
        mut self,
        selection: crate::raw::TextSelection,
        on_change: impl Fn(crate::raw::TextSelection) + 'static,
    ) -> Self {
        self.inner = self.inner.selection(selection, on_change);
        self
    }

    pub fn selection_background(mut self, color: creamui_theme::Color) -> Self {
        self.inner = self.inner.selection_background(color);
        self
    }

    pub fn selection_text_color(mut self, color: creamui_theme::Color) -> Self {
        self.inner = self.inner.selection_text_color(color);
        self
    }

    pub fn on_ctrl_o(mut self, callback: impl Fn() + 'static) -> Self {
        self.inner = self.inner.on_ctrl_o(callback);
        self
    }

    pub fn clipboard_enabled(mut self, enabled: bool) -> Self {
        self.inner = self.inner.clipboard_enabled(enabled);
        self
    }

    /// When `true`, long lines wrap onto a new row at the editor's width
    /// instead of scrolling past it. Off by default.
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.inner = self.inner.wrap(wrap);
        self
    }

    /// A `TextArea` whose value, cursor, and selection are all read from and
    /// written back to a [`crate::TextController`] — the multi-line
    /// counterpart of [`TextInput::controlled`], and the one place this
    /// pays off most: no more separately wiring `cursor`/`on_cursor_change`
    /// and `selection`/`on_selection_change` by hand. The controller must be
    /// a handle the app keeps alive across renders (created once, e.g. in
    /// `main`, the same way a `Signal` is).
    pub fn controlled(controller: &crate::TextController) -> Self {
        Self::controlled_with_style(Self::default_style(), controller)
    }

    /// Same as [`TextArea::controlled`], but with full control over layout.
    pub fn controlled_with_style(style: Style, controller: &crate::TextController) -> Self {
        let value_set = controller.clone();
        let cursor_set = controller.clone();
        let selection_set = controller.clone();
        Self::from_layout(style, controller.value(), move |next| {
            value_set.set_value(next)
        })
        .cursor(controller.cursor(), move |next| cursor_set.set_cursor(next))
        .selection(controller.selection(), move |next| {
            selection_set.set_selection(next)
        })
    }
}

impl Widget for TextArea {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect)
    }
    fn focusable(&self) -> bool {
        self.inner.focusable()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.inner.on_drag()
    }
    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.inner.on_drag_start()
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }
    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, caret_visible: bool) {
        self.inner
            .paint_focused_overlay(painter, rect, caret_visible)
    }
}

/// A themed horizontal slider.
pub struct Slider {
    inner: RawSlider,
}
impl_styled_inner!(Slider);

impl Slider {
    /// The style used when none is given explicitly: a fixed 160x20 box,
    /// matching this widget's original hardcoded layout.
    pub fn default_style() -> Style {
        Style {
            size: creamui_core::layout::Size {
                width: creamui_core::layout::Dimension::Length(160.0),
                height: creamui_core::layout::Dimension::Length(20.0),
            },
            ..Default::default()
        }
    }

    pub fn new(value: f32, on_change: impl Fn(f32) + 'static) -> Self {
        Self::from_layout(Self::default_style(), value, on_change)
    }

    /// Same as [`Slider::new`], but with full control over layout instead
    /// of the fixed 160x20 default.
    fn from_layout(style: Style, value: f32, on_change: impl Fn(f32) + 'static) -> Self {
        let theme = use_theme();
        let inner = RawSlider::new(
            style,
            value,
            theme.border_strong,
            theme.accent,
            theme.accent,
            on_change,
        );
        Slider { inner }
    }

    pub fn customize(mut self, customize: impl FnOnce(&mut RawSlider)) -> Self {
        customize(&mut self.inner);
        self
    }
}

impl Widget for Slider {
    fn focusable(&self) -> bool {
        self.inner.focusable()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }
    fn paint_focused_overlay(&self, p: &mut dyn Painter, r: Rect, c: bool) {
        self.inner.paint_focused_overlay(p, r, c);
    }
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        self.inner.on_drag()
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }
}
