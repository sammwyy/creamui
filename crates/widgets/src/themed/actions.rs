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
                Style::default()
                    .flex_row()
                    .align(Align::Center)
                    .padding_xy(14.0, 0.0),
                move || on_change(!pressed),
            )
            .pressed_state(pressed)
            .height(34.0)
            .background(if pressed {
                elevation::with_alpha(theme.accent, 46)
            } else {
                elevation::control_fill(&theme)
            })
            .border(
                if pressed {
                    elevation::with_alpha(theme.accent, 110)
                } else {
                    elevation::control_border(&theme)
                },
                1.0,
            )
            .box_shadow(
                elevation::control_shadow(&theme)
                    .filter(|_| !pressed)
                    .unwrap_or(creamui_core::BoxShadow::new(
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        elevation::clear(),
                    )),
            )
            .corner_radius(theme.button_radius)
            .pressed_style(creamui_core::StateStyle::new().background(if pressed {
                elevation::with_alpha(theme.accent, 70)
            } else {
                elevation::control_pressed(&theme)
            }))
            .hover_style(creamui_core::StateStyle::new().background(if pressed {
                elevation::with_alpha(theme.accent, 58)
            } else {
                elevation::control_hover(&theme)
            }))
            .focus_style(
                creamui_core::StateStyle::new().outline(elevation::focus_ring(&theme), 2.0),
            )
            .child(Box::new(
                RawText::new(
                    label,
                    if pressed {
                        elevation::tinted_text(&theme, theme.accent)
                    } else {
                        theme.text_primary
                    },
                    theme.typography.body,
                )
                .bold(pressed),
            )),
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
        let menu_background = theme.accent.mix(Color::rgb(0, 0, 0), 0.08);
        let foreground = if theme.selection_text.contrast_ratio(theme.accent) >= 4.5 {
            theme.selection_text
        } else {
            theme.accent.readable_text()
        };
        let menu_foreground = if foreground.contrast_ratio(menu_background) >= 4.5 {
            foreground
        } else {
            menu_background.readable_text()
        };
        let control = creamui_core::Style::new()
            .layout(Style {
                min_size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Length(34.0),
                },
                align_items: Some(creamui_core::layout::AlignItems::Center),
                justify_content: Some(creamui_core::layout::JustifyContent::Center),
                ..Default::default()
            })
            .background(elevation::with_alpha(theme.accent, 0))
            .corner_radius(0.0)
            .padding_edges(creamui_core::EdgeValues::new(
                creamui_core::LengthValue::Px(theme.spacing_medium),
                creamui_core::LengthValue::Px(theme.spacing_large),
                creamui_core::LengthValue::Px(theme.spacing_medium),
                creamui_core::LengthValue::Px(theme.spacing_large),
            ))
            .hover(
                creamui_core::StateStyle::new()
                    .background(elevation::with_alpha(Color::rgb(255, 255, 255), 24)),
            )
            .pressed(
                creamui_core::StateStyle::new()
                    .background(elevation::with_alpha(Color::rgb(0, 0, 0), 30)),
            )
            .focus(creamui_core::StateStyle::new().outline(elevation::focus_ring(&theme), 2.0));
        let menu = creamui_core::Style::new()
            .layout(Style {
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(34.0),
                    height: Dimension::Length(34.0),
                },
                align_items: Some(creamui_core::layout::AlignItems::Center),
                justify_content: Some(creamui_core::layout::JustifyContent::Center),
                ..Default::default()
            })
            .background(elevation::with_alpha(Color::rgb(0, 0, 0), 20))
            .corner_radius(0.0)
            .hover(
                creamui_core::StateStyle::new()
                    .background(elevation::with_alpha(Color::rgb(255, 255, 255), 24)),
            )
            .pressed(
                creamui_core::StateStyle::new()
                    .background(elevation::with_alpha(Color::rgb(0, 0, 0), 40)),
            )
            .focus(creamui_core::StateStyle::new().outline(elevation::focus_ring(&theme), 2.0));
        Self {
            inner: RawDropdownButton::new(
                Style {
                    min_size: creamui_core::layout::Size {
                        width: Dimension::Auto,
                        height: Dimension::Length(34.0),
                    },
                    ..Style::default().flex_row().gap(0.0)
                },
                Box::new(RawText::new(label, foreground, theme.typography.body).bold(true)),
                Box::new(crate::Icon::new(crate::Symbol::ChevronDown, menu_foreground).size(14.0)),
                on_primary,
                on_menu,
            )
            .button_styles(control, menu)
            .background(theme.accent)
            .corner_radius(theme.button_radius),
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.inner = self.inner.disabled(disabled);
        self
    }
}

impl Widget for DropdownButton {
    fn clips_children(&self) -> bool {
        true
    }

    fn clip_corner_radius(&self) -> f32 {
        self.inner.style.paint.corner_radius.unwrap_or(0.0)
    }

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
                    height: Dimension::Length(34.0),
                },
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Auto,
                },
                flex_grow: 1.0,
                ..Default::default()
            })
            .background(elevation::with_alpha(theme.text_primary, 0))
            .border(elevation::with_alpha(theme.text_primary, 0), 0.0)
            .hover_style(creamui_core::StateStyle::new())
            .focus_style(creamui_core::StateStyle::new())
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
            .background(elevation::field_fill(&theme))
            .border(elevation::control_border(&theme), theme.input_border_width)
            .corner_radius(theme.input_radius),
        }
    }

    pub fn prefix(mut self, text: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.leading(Box::new(
            RawText::new(text, theme.text_secondary, theme.typography.body)
                .padding_left(theme.spacing_medium)
                .padding_right(theme.spacing_medium),
        ));
        self
    }

    pub fn suffix(mut self, text: impl Into<String>) -> Self {
        let theme = use_theme();
        self.inner = self.inner.trailing(Box::new(
            RawText::new(text, theme.text_secondary, theme.typography.body)
                .padding_left(theme.spacing_medium)
                .padding_right(theme.spacing_medium),
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

    #[test]
    fn themed_actions_have_distinct_control_and_menu_geometry() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let theme = use_theme();
            let toggle = ToggleButton::new("Pin", false, |_| {});
            assert_eq!(
                toggle.style().layout.padding.left,
                creamui_core::layout::LengthPercentage::Length(14.0)
            );
            assert!(toggle.style().paint.border.is_some());
            assert!(toggle.style().states.hover.paint.background.is_some());

            let mut split = DropdownButton::new("Save", || {}, || {});
            assert!(split.clips_children());
            assert_eq!(split.clip_corner_radius(), theme.button_radius);
            assert_eq!(split.style().paint.background, Some(theme.accent.into()));
            assert_eq!(
                split.style().layout.gap.width,
                creamui_core::layout::LengthPercentage::Length(0.0)
            );
            let buttons = split.children();
            assert_eq!(
                buttons[0].style().layout.padding.left,
                creamui_core::layout::LengthPercentage::Length(theme.spacing_large)
            );
            assert_eq!(
                buttons[1].style().layout.min_size.width,
                Dimension::Length(34.0)
            );
            assert!(buttons.iter().all(|button| button.focusable()));
            assert!(buttons
                .iter()
                .all(|button| button.style().paint.corner_radius == Some(0.0)));
            assert!(buttons.iter().all(|button| button.on_key().is_some()));
        });
    }

    #[test]
    fn themed_split_button_actions_remain_independent() {
        creamui_reactive::with_context_scope(|| {
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::default());
            let clicked = Rc::new(std::cell::Cell::new((0, 0)));
            let primary = clicked.clone();
            let menu = clicked.clone();
            let mut split = DropdownButton::new(
                "Save",
                move || primary.set((primary.get().0 + 1, primary.get().1)),
                move || menu.set((menu.get().0, menu.get().1 + 1)),
            );
            let buttons = split.children();
            buttons[0].on_click().unwrap()();
            buttons[1].on_click().unwrap()();
            assert_eq!(clicked.get(), (1, 1));
        });
    }
}
