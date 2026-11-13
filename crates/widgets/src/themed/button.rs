use super::*;
/// A themed, clickable button with a centered text label.
pub struct Button {
    inner: RawButton,
    theme: Theme,
    label: String,
    size: ButtonSize,
    state: ButtonState,
    enabled_children: Option<Vec<BoxedWidget>>,
}
impl_styled_inner!(Button);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonSize {
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
}
impl ButtonSize {
    fn padding(self) -> f32 {
        match self {
            Self::Xs => 8.,
            Self::Sm => 10.,
            Self::Md => 14.,
            Self::Lg => 18.,
            Self::Xl => 22.,
        }
    }
    fn font_size(self) -> f32 {
        match self {
            Self::Xs => 12.,
            Self::Sm => 13.,
            Self::Md => 14.,
            Self::Lg => 16.,
            Self::Xl => 18.,
        }
    }
    fn border_width(self) -> f32 {
        1.
    }
    fn height(self) -> f32 {
        match self {
            Self::Xs => 24.,
            Self::Sm => 28.,
            Self::Md => 34.,
            Self::Lg => 40.,
            Self::Xl => 48.,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonState {
    Normal,
    Loading,
    Success,
}
/// Visual hierarchy for native application actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Tertiary,
    Destructive,
    Success,
}

impl Button {
    /// A complete native-control button: variant, size and state are all
    /// semantic, so apps don't have to hand-pick raw rectangles.
    pub fn styled(
        variant: ButtonVariant,
        size: ButtonSize,
        label: impl Into<String>,
        state: ButtonState,
        on_click: impl Fn() + 'static,
    ) -> Self {
        let theme = use_theme();
        let (background, border, foreground) = match variant {
            ButtonVariant::Primary => (theme.accent, theme.accent, theme.selection_text),
            ButtonVariant::Secondary => (
                elevation::control_fill(&theme),
                elevation::control_border(&theme),
                theme.text_primary,
            ),
            ButtonVariant::Tertiary => (elevation::clear(), elevation::clear(), theme.text_primary),
            ButtonVariant::Destructive => (theme.danger, theme.danger, theme.selection_text),
            ButtonVariant::Success => (theme.success, theme.success, theme.selection_text),
        };
        let filled = matches!(
            variant,
            ButtonVariant::Primary | ButtonVariant::Destructive | ButtonVariant::Success
        );
        let label = label.into();
        let foreground = if state == ButtonState::Loading {
            theme.text_secondary
        } else {
            foreground
        };
        let backdrop = if background.a == 0 {
            elevation::opaque(&theme, theme.surface_elevated)
        } else {
            background
        };
        let foreground = if foreground.contrast_ratio(backdrop) < 4.5 {
            backdrop.readable_text()
        } else {
            foreground
        };
        let mut style = centered_box_style(size.padding());
        style.size.height =
            creamui_core::layout::Dimension::Length(size.height() * theme.text_scale);
        style.min_size.width =
            creamui_core::layout::Dimension::Length(size.height() * theme.text_scale);
        style.flex_shrink = 0.0;
        let text = RawText::new(
            label.clone(),
            foreground,
            size.font_size() * theme.text_scale,
        )
        .font_family(theme.font_family)
        .bold(filled);
        let child: BoxedWidget = if state != ButtonState::Normal {
            let content = Style {
                display: creamui_core::layout::Display::Flex,
                flex_direction: creamui_core::layout::FlexDirection::Row,
                align_items: Some(AlignItems::Center),
                gap: creamui_core::layout::Size {
                    width: LengthPercentage::Length(6.0),
                    height: LengthPercentage::Length(6.0),
                },
                ..Default::default()
            };
            let icon: BoxedWidget = if state == ButtonState::Loading {
                Box::new(RawSpinner::new(foreground).size(size.font_size()))
            } else {
                Box::new(crate::Icon::new(crate::Symbol::Check, foreground).size(size.font_size()))
            };
            Box::new(RawView::new(content).child(icon).child(Box::new(text)))
        } else {
            Box::new(text)
        };
        let shadow = (variant == ButtonVariant::Secondary)
            .then(|| elevation::control_shadow(&theme))
            .flatten();
        let mut inner = RawButton::new(style, on_click)
            .background(background)
            .border(border, size.border_width())
            .corner_radius(theme.button_radius.min(size.height() / 2.))
            .child(child);
        if let Some(shadow) = shadow {
            inner = inner.box_shadow(shadow);
        }
        let (hover_background, pressed_background) = match variant {
            ButtonVariant::Primary => (theme.accent_hover, theme.accent_pressed),
            ButtonVariant::Secondary => (
                elevation::control_hover(&theme),
                elevation::control_pressed(&theme),
            ),
            ButtonVariant::Tertiary => (elevation::subtle(&theme, 1), elevation::subtle(&theme, 2)),
            ButtonVariant::Destructive | ButtonVariant::Success => (
                background.mix(Color::rgb(255, 255, 255), 0.08),
                background.mix(Color::rgb(0, 0, 0), 0.1),
            ),
        };
        let mut hover = creamui_core::StateStyle::new().background(hover_background);
        let mut pressed = creamui_core::StateStyle::new().background(pressed_background);
        if filled {
            hover = hover.border(hover_background, size.border_width());
            pressed = pressed.border(pressed_background, size.border_width());
        }
        let no_shadow = creamui_core::BoxShadow::new(0.0, 0.0, 0.0, 0.0, elevation::clear());
        if shadow.is_some() {
            pressed = pressed.box_shadow(no_shadow);
        }
        let disabled = if state == ButtonState::Loading {
            creamui_core::StateStyle::new()
                .background(background)
                .border(border, 1.0)
        } else {
            creamui_core::StateStyle::new()
                .background(elevation::subtle(&theme, 1))
                .border(elevation::clear(), 1.0)
                .box_shadow(no_shadow)
        };
        inner = inner
            .hover_style(hover)
            .pressed_style(pressed)
            .focus_style(
                creamui_core::StateStyle::new().outline(elevation::focus_ring(&theme), 2.0),
            )
            .disabled_style(disabled);
        let mut button = Self {
            inner,
            theme,
            label,
            size,
            state,
            enabled_children: None,
        };
        if state == ButtonState::Loading {
            button.inner.disabled = true;
        }
        button
    }

    /// Sized primary button with built-in loading and success presentations.
    pub fn state(
        size: ButtonSize,
        label: impl Into<String>,
        state: ButtonState,
        on_click: impl Fn() + 'static,
    ) -> Self {
        Self::styled(ButtonVariant::Primary, size, label, state, on_click)
    }

    /// A neutral, still-clickable button for secondary actions.
    pub fn secondary(
        size: ButtonSize,
        label: impl Into<String>,
        on_click: impl Fn() + 'static,
    ) -> Self {
        Self::styled(
            ButtonVariant::Secondary,
            size,
            label,
            ButtonState::Normal,
            on_click,
        )
    }
    pub fn new(label: impl Into<String>, on_click: impl Fn() + 'static) -> Self {
        Self::styled(
            ButtonVariant::Primary,
            ButtonSize::Md,
            label,
            ButtonState::Normal,
            on_click,
        )
    }

    pub fn on_key_press(mut self, handler: impl Fn(KeyInput) + 'static) -> Self {
        self.inner = self.inner.on_key_press(handler);
        self
    }

    /// Disable activation and apply the shared muted control treatment.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.inner.disabled = disabled || self.state == ButtonState::Loading;
        if disabled && self.state != ButtonState::Loading && self.enabled_children.is_none() {
            self.enabled_children = Some(std::mem::take(&mut self.inner.children));
            self.inner.children = vec![Box::new(
                RawText::new(
                    self.label.clone(),
                    self.theme.text_disabled,
                    self.size.font_size() * self.theme.text_scale,
                )
                .font_family(self.theme.font_family),
            )];
        } else if !disabled {
            if let Some(children) = self.enabled_children.take() {
                self.inner.children = children;
            }
        }
        self
    }

    pub fn customize(mut self, customize: impl FnOnce(&mut RawButton)) -> Self {
        customize(&mut self.inner);
        self
    }
}

impl Widget for Button {
    fn style_state(&self) -> creamui_core::StyleState {
        self.inner.style_state()
    }
    fn focusable(&self) -> bool {
        self.inner.focusable()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        self.inner.on_key()
    }
    fn paint_focused_overlay(&self, painter: &mut dyn Painter, rect: Rect, caret: bool) {
        self.inner.paint_focused_overlay(painter, rect, caret);
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

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        self.inner.cursor_icon()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_states_derive_their_fills_from_theme_colors() {
        for theme in [Theme::light(), Theme::dark()] {
            creamui_reactive::with_context_scope(|| {
                creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
                let button = Button::new("Continue", || {});
                let style = button.style();
                assert_eq!(style.paint.background, Some(theme.accent.into()));
                assert_eq!(
                    style.states.hover.paint.background,
                    Some(theme.accent_hover.into())
                );
                assert!(style.paint.box_shadow.is_none());
                assert_eq!(
                    style.states.pressed.paint.background,
                    Some(theme.accent_pressed.into())
                );
            });
        }
    }

    #[test]
    fn high_contrast_buttons_keep_solid_fill_and_no_shadow() {
        creamui_reactive::with_context_scope(|| {
            let mut theme = Theme::light();
            theme.high_contrast = true;
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
            let button = Button::new("Continue", || {});
            assert_eq!(button.style().paint.background, Some(theme.accent.into()));
            assert!(button.style().paint.box_shadow.is_none());
            assert!(button.style().states.pressed.paint.box_shadow.is_none());
        });
    }

    #[test]
    fn only_neutral_buttons_cast_a_shadow_and_only_in_light_themes() {
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::light();
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
            let secondary = Button::secondary(ButtonSize::Xs, "Action", || {});
            assert!(secondary.style().paint.box_shadow.is_some());
            let primary = Button::styled(
                ButtonVariant::Primary,
                ButtonSize::Sm,
                "Action",
                ButtonState::Normal,
                || {},
            );
            assert!(primary.style().paint.box_shadow.is_none());
            let tertiary = Button::styled(
                ButtonVariant::Tertiary,
                ButtonSize::Md,
                "Action",
                ButtonState::Normal,
                || {},
            );
            let style = tertiary.style();
            assert!(style.paint.box_shadow.is_none());
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(Theme::dark()));
            assert!(Button::secondary(ButtonSize::Md, "Action", || {})
                .style()
                .paint
                .box_shadow
                .is_none());
            let mut tertiary = tertiary;
            let label = tertiary.children().remove(0);
            let color = label
                .style()
                .typography
                .color
                .unwrap()
                .resolve(&theme.colors);
            assert_eq!(color, theme.text_primary);
        });
    }

    #[test]
    fn action_labels_contrast_with_custom_accents_without_overriding_explicit_styles() {
        for accent in [Color::rgb(12, 24, 48), Color::rgb(242, 224, 140)] {
            creamui_reactive::with_context_scope(|| {
                let theme = Theme::light().with_accent(accent);
                creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
                let mut button = Button::new("Continue", || {});
                let label = button.children().remove(0);
                let color = label
                    .style()
                    .typography
                    .color
                    .unwrap()
                    .resolve(&theme.colors);
                assert!(color.contrast_ratio(accent) >= 4.5);
                let custom = Color::rgb(65, 45, 95);
                let button = Button::new("Continue", || {}).background(custom);
                assert_eq!(button.style().paint.background, Some(custom.into()));
                assert!(button.style().paint.box_shadow.is_none());
            });
        }
    }

    #[test]
    fn loading_button_keeps_a_legible_background_while_noninteractive() {
        creamui_reactive::with_context_scope(|| {
            let theme = Theme::dark().with_accent(Color::rgb(181, 139, 255));
            creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
            let mut button = Button::state(ButtonSize::Md, "Working", ButtonState::Loading, || {});
            assert!(!button.focusable());
            let fill = button
                .style()
                .resolve(creamui_core::StyleState::NORMAL.with_disabled(true))
                .paint
                .background
                .unwrap();
            assert_eq!(fill, theme.accent.into());
            let mut content = button.children().remove(0);
            let label = content.children().remove(1);
            let text_color = label
                .style()
                .typography
                .color
                .unwrap()
                .resolve(&theme.colors);
            assert!(text_color.contrast_ratio(theme.accent) >= 4.5);
        });
    }
}
