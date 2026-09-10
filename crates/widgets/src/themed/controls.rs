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
        let mut inner =
            RawCheckbox::new(18.0, checked, theme.accent, theme.border_strong, on_click);
        inner = inner.corner_radius(theme.checkbox_radius);
        Checkbox { inner }
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
        Self {
            inner: RawSwitch::new(
                checked,
                theme.accent,
                theme.border_strong,
                theme.selection_text,
                on_click,
            ),
        }
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
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }
}
