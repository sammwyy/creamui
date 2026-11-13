use super::*;
use crate::layout::{Align, StyleExt};
use crate::raw::{RawDropdownButton, RawInputGroup};

pub struct ToggleButton {
    inner: RawButton,
}
impl_styled_inner!(ToggleButton);

impl ToggleButton {
    pub fn new(
        label: impl Into<String>,
        pressed: bool,
        on_change: impl Fn(bool) + 'static,
    ) -> Self {
        let theme = use_theme();
        Self {
            inner: RawButton::new(
                Style::default().padding_xy(theme.spacing_medium, theme.spacing_small),
                move || on_change(!pressed),
            )
            .pressed_state(pressed)
            .background(if pressed {
                theme.accent
            } else {
                theme.surface_elevated
            })
            .corner_radius(theme.button_radius)
            .pressed_style(creamui_core::StateStyle::new().background(theme.accent))
            .focus_style(creamui_core::StateStyle::new().outline(theme.accent, 2.0))
            .child(Box::new(RawText::new(
                label,
                if pressed {
                    theme.selection_text
                } else {
                    theme.text_primary
                },
                theme.typography.body,
            ))),
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.inner = self.inner.disabled(disabled);
        self
    }
}

impl Widget for ToggleButton {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn style_state(&self) -> creamui_core::StyleState {
        self.inner.style_state()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }

    fn focusable(&self) -> bool {
        self.inner.focusable()
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }
}

pub struct DropdownButton {
    inner: RawDropdownButton,
}
impl_styled_inner!(DropdownButton);

impl DropdownButton {
    pub fn new(
        label: impl Into<String>,
        on_primary: impl Fn() + 'static,
        on_menu: impl Fn() + 'static,
    ) -> Self {
        let theme = use_theme();
        let control = creamui_core::Style::new()
            .background(theme.accent)
            .corner_radius(theme.button_radius)
            .padding(theme.spacing_small);
        let menu = creamui_core::Style::new()
            .background(theme.accent)
            .corner_radius(theme.button_radius)
            .padding(theme.spacing_small);
        Self {
            inner: RawDropdownButton::new(
                Style::default().flex_row().align(Align::Center).gap(2.0),
                Box::new(RawText::new(
                    label,
                    theme.selection_text,
                    theme.typography.body,
                )),
                Box::new(RawText::new(
                    "...",
                    theme.selection_text,
                    theme.typography.body,
                )),
                on_primary,
                on_menu,
            )
            .button_styles(control, menu),
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.inner = self.inner.disabled(disabled);
        self
    }
}

impl Widget for DropdownButton {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
}

pub struct InputGroup {
    inner: RawInputGroup,
}
impl_styled_inner!(InputGroup);

impl InputGroup {
    pub fn new(value: impl Into<String>, on_change: impl Fn(String) + 'static) -> Self {
        Self::with_input(TextInput::new(value, on_change))
    }

    pub fn controlled(controller: &crate::TextController) -> Self {
        Self::with_input(TextInput::controlled(controller))
    }

    fn with_input(input: TextInput) -> Self {
        let theme = use_theme();
        let input = input
            .layout(Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Length(36.0),
                },
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Auto,
                },
                flex_grow: 1.0,
                ..Default::default()
            })
            .border(theme.surface_elevated, 0.0)
            .corner_radius(0.0);
        Self {
            inner: RawInputGroup::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Auto,
                    },
                    max_size: creamui_core::layout::Size {
                        width: Dimension::Length(400.0),
                        height: Dimension::Auto,
                    },
                    min_size: creamui_core::layout::Size {
                        width: Dimension::Length(0.0),
                        height: Dimension::Auto,
                    },
                    ..Style::default().flex_row().align(Align::Center)
                },
                Box::new(input),
            )
            .background(theme.surface_elevated)
            .border(theme.border, theme.input_border_width)
            .corner_radius(theme.input_radius),
        }
    }

    pub fn prefix(mut self, text: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.leading(Box::new(
            RawText::new(text, theme.text_secondary, theme.typography.body)
                .padding(theme.spacing_small),
        ));
        self
    }

    pub fn suffix(mut self, text: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.trailing(Box::new(
            RawText::new(text, theme.text_secondary, theme.typography.body)
                .padding(theme.spacing_small),
        ));
        self
    }

    pub fn leading(mut self, widget: BoxedWidget) -> Self {
        self.inner = self.inner.leading(widget);
        self
    }

    pub fn trailing(mut self, widget: BoxedWidget) -> Self {
        self.inner = self.inner.trailing(widget);
        self
    }
}

impl Widget for InputGroup {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themed_actions_preserve_callbacks_and_slots() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let pressed = Rc::new(std::cell::Cell::new(false));
            let set_pressed = pressed.clone();
            let toggle = ToggleButton::new("Toggle", false, move |next| set_pressed.set(next));
            toggle.on_click().unwrap()();
            assert!(pressed.get());
            let mut split = DropdownButton::new("Save", || {}, || {});
            assert_eq!(split.children().len(), 2);
            let mut group = InputGroup::new("", |_| {}).prefix("$").suffix("USD");
            assert_eq!(group.children().len(), 3);
        });
    }
}
