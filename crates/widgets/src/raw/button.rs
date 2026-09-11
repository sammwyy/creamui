use super::*;

/// An unstyled clickable region. Its common style supplies the box paint;
/// combine it with [`RawText`] as a child for a labeled button.
pub struct RawButton {
    pub style: creamui_core::Style,
    pub children: Vec<BoxedWidget>,
    pub on_click: Rc<dyn Fn()>,
    pub on_click_at: Option<Rc<dyn Fn(Point)>>,
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

    fn paint_fingerprint(&self) -> Option<u64> {
        use std::hash::{Hash, Hasher};
        // Hover/press are covered by `paint_instance`'s `cached_states`
        // guard; resolving against `NORMAL` here only needs to catch a real
        // change to the button's own background/border/outline.
        let paint = self.style.resolve(creamui_core::StyleState::NORMAL).paint;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.disabled.hash(&mut hasher);
        paint.background.hash(&mut hasher);
        paint
            .border
            .map(|b| (b.color, b.width.to_bits()))
            .hash(&mut hasher);
        paint.corner_radius.map(f32::to_bits).hash(&mut hasher);
        paint
            .outline
            .map(|o| (o.color, o.width.to_bits()))
            .hash(&mut hasher);
        Some(hasher.finish())
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

    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        if self.disabled {
            None
        } else {
            self.on_click_at.clone()
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

#[cfg(test)]
mod raw_button_fingerprint_tests {
    use super::*;

    #[test]
    fn identical_background_hashes_equal() {
        let a = RawButton::new(
            creamui_core::Style::new().background(Color::rgb(1, 2, 3)),
            || {},
        );
        let b = RawButton::new(
            creamui_core::Style::new().background(Color::rgb(1, 2, 3)),
            || {},
        );
        assert_eq!(a.paint_fingerprint(), b.paint_fingerprint());
    }

    #[test]
    fn different_background_hashes_differently() {
        let a = RawButton::new(
            creamui_core::Style::new().background(Color::rgb(1, 2, 3)),
            || {},
        );
        let b = RawButton::new(
            creamui_core::Style::new().background(Color::rgb(4, 5, 6)),
            || {},
        );
        assert_ne!(a.paint_fingerprint(), b.paint_fingerprint());
    }

    #[test]
    fn different_corner_radius_hashes_differently() {
        let a = RawButton::new(
            creamui_core::Style::new()
                .background(Color::rgb(1, 2, 3))
                .corner_radius(4.0),
            || {},
        );
        let b = RawButton::new(
            creamui_core::Style::new()
                .background(Color::rgb(1, 2, 3))
                .corner_radius(8.0),
            || {},
        );
        assert_ne!(a.paint_fingerprint(), b.paint_fingerprint());
    }

    #[test]
    fn layout_only_change_does_not_affect_fingerprint() {
        let a = RawButton::new(
            creamui_core::Style::new()
                .background(Color::rgb(1, 2, 3))
                .width(10.0),
            || {},
        );
        let b = RawButton::new(
            creamui_core::Style::new()
                .background(Color::rgb(1, 2, 3))
                .width(200.0),
            || {},
        );
        assert_eq!(a.paint_fingerprint(), b.paint_fingerprint());
    }
}
