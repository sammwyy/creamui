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
            Self::Xs => 6.,
            Self::Sm => 9.,
            Self::Md => 12.,
            Self::Lg => 16.,
            Self::Xl => 20.,
        }
    }
    fn font_size(self) -> f32 {
        match self {
            Self::Xs => 11.,
            Self::Sm => 12.,
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
            ButtonVariant::Primary => (theme.accent, theme.accent_hover, theme.selection_text),
            ButtonVariant::Secondary => (
                theme.surface_elevated,
                theme.border_strong,
                theme.text_primary,
            ),
            ButtonVariant::Tertiary => (theme.surface, theme.surface, theme.text_primary),
            ButtonVariant::Destructive => (theme.danger, theme.danger, theme.selection_text),
            ButtonVariant::Success => (theme.success, theme.success, theme.selection_text),
        };
        let label = match state {
            ButtonState::Success => format!("✓ {}", label.into()),
            _ => label.into(),
        };
        let mut style = centered_box_style(size.padding());
        style.size.height = creamui_core::layout::Dimension::Length(size.height());
        let text = RawText::new(label.clone(), foreground, size.font_size());
        let child: BoxedWidget = if state == ButtonState::Loading {
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
            Box::new(
                RawView::new(content)
                    .child(Box::new(RawSpinner::new(foreground).size(size.font_size())))
                    .child(Box::new(text)),
            )
        } else {
            Box::new(text)
        };
        let mut inner = RawButton::new(style, on_click)
            .background(background)
            .border(border, size.border_width())
            .corner_radius(theme.button_radius.min(size.height() / 4.))
            .child(child);
        inner = inner
            .hover_background(if variant == ButtonVariant::Primary {
                theme.accent_hover
            } else {
                background.mix(theme.text_primary, 0.07)
            })
            .pressed_background(if variant == ButtonVariant::Primary {
                theme.accent_pressed
            } else {
                background.mix(theme.text_primary, 0.14)
            })
            .focus_color(theme.accent);
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

    /// A themed button with caller-controlled layout. Its colors and radius
    /// still come from `theme`, so an application-wide theme change remains
    /// consistent while each button can choose its own size, margin, or flex
    /// placement.
    pub fn with_style(
        style: Style,
        label: impl Into<String>,
        on_click: impl Fn() + 'static,
    ) -> Self {
        let mut button = Self::new(label, on_click);
        button.inner.style.layout = style;
        button
    }

    /// Disable activation and apply the shared muted control treatment.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.inner.disabled = disabled || self.state == ButtonState::Loading;
        if disabled && self.enabled_children.is_none() {
            self.enabled_children = Some(std::mem::take(&mut self.inner.children));
            self.inner.children = vec![Box::new(RawText::new(
                self.label.clone(),
                self.theme.text_disabled,
                self.size.font_size(),
            ))];
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
        if self.enabled_children.is_some() {
            let radius = self
                .inner
                .style_declaration()
                .paint
                .corner_radius
                .unwrap_or(0.0);
            painter.fill_rect(rect, self.theme.surface_hover, radius);
            painter.stroke_rect(rect, self.theme.border, 1., radius);
        } else {
            self.inner.paint(painter, rect);
        }
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
