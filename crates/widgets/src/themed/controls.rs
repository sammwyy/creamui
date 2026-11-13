use super::*;
/// A themed checkbox: filled with the theme's accent color when checked,
/// outlined with its border color otherwise.
pub struct Checkbox {
    inner: RawCheckbox,
}
impl_styled_inner!(Checkbox);

impl Checkbox {
    pub fn new(checked: bool, on_click: impl Fn() + 'static) -> Self {
        let theme = use_theme();
        let inner = RawCheckbox::new(
            18.0,
            checked,
            theme.accent,
            elevation::with_alpha(theme.text_primary, 90),
            on_click,
        )
        .check_color(theme.accent.readable_text())
        .check_width(2.0)
        .corner_radius(theme.checkbox_radius)
        .background(if checked {
            elevation::clear()
        } else {
            elevation::field_fill(&theme)
        })
        .hover_style(
            creamui_core::StateStyle::new()
                .border(elevation::with_alpha(theme.text_primary, 140), 1.5),
        )
        .focus_style(
            creamui_core::StateStyle::new()
                .outline(elevation::focus_ring(&theme), elevation::FOCUS_RING_WIDTH),
        );
        Checkbox { inner }
    }

    pub fn transition(mut self, transition: creamui_core::Transition) -> Self {
        self.inner = self.inner.transition(transition);
        self
    }

    pub fn customize(mut self, customize: impl FnOnce(&mut RawCheckbox)) -> Self {
        customize(&mut self.inner);
        self
    }
}

impl Widget for Checkbox {
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

    fn has_transition(&self) -> bool {
        self.inner.has_transition()
    }
    fn paint_transition(
        &self,
        painter: &mut dyn Painter,
        rect: Rect,
        content: Rect,
        state: &mut creamui_core::TransitionState,
    ) {
        self.inner.paint_transition(painter, rect, content, state);
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }
}

pub struct Spinner {
    inner: RawSpinner,
}
impl_styled_inner!(Spinner);
impl Spinner {
    pub fn new() -> Self {
        let theme = use_theme();
        Self {
            inner: RawSpinner::new(theme.accent),
        }
    }
    pub fn phase(mut self, phase: usize) -> Self {
        self.inner = self.inner.phase(phase);
        self
    }
    pub fn size(mut self, size: f32) -> Self {
        self.inner = self.inner.size(size);
        self
    }

    pub fn customize(mut self, customize: impl FnOnce(&mut RawSpinner)) -> Self {
        customize(&mut self.inner);
        self
    }
}
impl Widget for Spinner {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect)
    }
}

/// A compact sliding boolean control, complementary to [`Checkbox`].
pub struct Switch {
    inner: RawSwitch,
}
impl_styled_inner!(Switch);
impl Switch {
    pub fn new(checked: bool, on_click: impl Fn() + 'static) -> Self {
        let theme = use_theme();
        let off = elevation::rail_fill(&theme);
        let mut inner = RawSwitch::new(
            checked,
            theme.accent,
            off,
            Color::rgb(255, 255, 255),
            on_click,
        )
        .hover_colors(
            theme.accent_hover,
            elevation::with_alpha(off, off.a.saturating_add(14)),
        )
        .focus_style(
            creamui_core::StateStyle::new()
                .outline(elevation::focus_ring(&theme), elevation::FOCUS_RING_WIDTH),
        );
        if !theme.high_contrast {
            inner = inner.thumb_shadow(elevation::knob_shadow(&theme));
        }
        Self { inner }
    }

    pub fn transition(mut self, transition: creamui_core::Transition) -> Self {
        self.inner = self.inner.transition(transition);
        self
    }

    pub fn customize(mut self, customize: impl FnOnce(&mut RawSwitch)) -> Self {
        customize(&mut self.inner);
        self
    }
}
impl Widget for Switch {
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
        self.inner.paint(painter, rect)
    }
    fn has_transition(&self) -> bool {
        self.inner.has_transition()
    }
    fn paint_transition(
        &self,
        painter: &mut dyn Painter,
        rect: Rect,
        content: Rect,
        state: &mut creamui_core::TransitionState,
    ) {
        self.inner.paint_transition(painter, rect, content, state);
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }
}
