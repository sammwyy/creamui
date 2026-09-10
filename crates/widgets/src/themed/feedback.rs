use super::*;
use crate::layout::{column, fixed, padding, row};
use creamui_core::layout::{
    AlignItems, Dimension, JustifyContent, LengthPercentageAuto, Position, Style,
};

/// An invisible portal-sized hit target placed behind a transient popup.
///
/// Absolute children are painted in CreamUI's portal pass, so a normal
/// full-parent overlay would only cover the trigger's small layout box. This
/// deliberately oversized layer covers the window instead, allowing a popup
/// to dismiss reliably when its user clicks anywhere outside it.
pub(crate) fn portal_dismiss_layer(on_dismiss: impl Fn() + 'static) -> BoxedWidget {
    const EXTENT: f32 = 1_000_000.0;
    Box::new(RawButton::new(
        Style {
            position: Position::Absolute,
            inset: creamui_core::layout::Rect {
                left: LengthPercentageAuto::Length(-EXTENT),
                right: LengthPercentageAuto::Auto,
                top: LengthPercentageAuto::Length(-EXTENT),
                bottom: LengthPercentageAuto::Auto,
            },
            size: creamui_core::layout::Size {
                width: Dimension::Length(EXTENT * 2.0),
                height: Dimension::Length(EXTENT * 2.0),
            },
            ..Default::default()
        },
        on_dismiss,
    ))
}

/// A full-parent dimmer used as the base of modal dialogs and transient
/// overlays. Place it after ordinary application content so it paints and
/// receives hit testing above that content.
pub struct Overlay {
    style: Style,
    dismiss: Rc<dyn Fn()>,
    children: Vec<BoxedWidget>,
}

impl Overlay {
    pub fn new(style: Style, on_dismiss: impl Fn() + 'static) -> Self {
        Self {
            style,
            dismiss: Rc::new(on_dismiss),
            children: Vec::new(),
        }
    }

    /// A flex-centered overlay that fills its positioned parent.
    pub fn fullscreen(on_dismiss: impl Fn() + 'static) -> Self {
        let style = Style {
            position: Position::Absolute,
            inset: creamui_core::layout::Rect {
                left: creamui_core::layout::LengthPercentageAuto::Length(0.0),
                right: creamui_core::layout::LengthPercentageAuto::Length(0.0),
                top: creamui_core::layout::LengthPercentageAuto::Length(0.0),
                bottom: creamui_core::layout::LengthPercentageAuto::Length(0.0),
            },
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Percent(1.0),
            },
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            ..column(0.0)
        };
        Self::new(style, on_dismiss)
    }

    pub fn child(mut self, child: BoxedWidget) -> Self {
        self.children.push(child);
        self
    }

    pub fn with_children(mut self, children: Vec<BoxedWidget>) -> Self {
        self.children = children;
        self
    }
}

impl Widget for Overlay {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        // The alpha deliberately leaves enough of the surrounding app visible
        // to preserve context while making the active layer unambiguous.
        painter.fill_rect(rect, Color::rgba(0, 0, 0, 112), 0.0);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        Some(self.dismiss.clone())
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Default)
    }
}

/// A floating, click-shielding surface. Its own empty click handler prevents
/// an enclosing [`Overlay`] from treating clicks inside the popover as an
/// outside dismissal; interactive descendants still win hit testing.
pub struct Popover {
    theme: Theme,
    style: Style,
    children: Vec<BoxedWidget>,
}

impl Popover {
    pub fn new(style: Style) -> Self {
        let theme = use_theme();
        Self {
            theme,
            style,
            children: Vec::new(),
        }
    }
    pub fn child(mut self, child: BoxedWidget) -> Self {
        self.children.push(child);
        self
    }
    pub fn with_children(mut self, children: Vec<BoxedWidget>) -> Self {
        self.children = children;
        self
    }
}

impl Widget for Popover {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        for spread in (1..=5).rev() {
            let spread = spread as f32;
            painter.fill_rect(
                Rect {
                    x: rect.x - spread,
                    y: rect.y - spread + 3.0,
                    width: rect.width + spread * 2.0,
                    height: rect.height + spread * 2.0,
                },
                Color::rgba(0, 0, 0, 5),
                self.theme.menu_radius + spread,
            );
        }
        painter.fill_rect(rect, self.theme.surface_elevated, self.theme.menu_radius);
        painter.stroke_rect(rect, self.theme.border_strong, 1.0, self.theme.menu_radius);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        Some(Rc::new(|| {}))
    }
}

/// A modal dialog. The application keeps its visibility in a `Signal` and
/// conditionally includes this widget in its root tree.
pub struct Dialog {
    theme: Theme,
    title: String,
    message: String,
    dismiss: Rc<dyn Fn()>,
    actions: Vec<(String, ButtonVariant, Rc<dyn Fn()>)>,
}

impl Dialog {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        on_dismiss: impl Fn() + 'static,
    ) -> Self {
        let theme = use_theme();
        Self {
            theme,
            title: title.into(),
            message: message.into(),
            dismiss: Rc::new(on_dismiss),
            actions: Vec::new(),
        }
    }

    pub fn action(
        mut self,
        label: impl Into<String>,
        variant: ButtonVariant,
        on_click: impl Fn() + 'static,
    ) -> Self {
        self.actions
            .push((label.into(), variant, Rc::new(on_click)));
        self
    }

    pub fn dismiss_action(mut self, label: impl Into<String>) -> Self {
        let dismiss = self.dismiss.clone();
        self.actions
            .push((label.into(), ButtonVariant::Secondary, dismiss));
        self
    }

    fn overlay_style() -> Style {
        Overlay::fullscreen(|| {}).style
    }
}

impl Widget for Dialog {
    fn style(&self) -> creamui_core::Style {
        Self::overlay_style().into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.fill_rect(rect, Color::rgba(0, 0, 0, 112), 0.0);
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        let theme = self.theme;
        let title = self.title.clone();
        let message = self.message.clone();
        let actions_data = std::mem::take(&mut self.actions);
        {
            let card_style = padding(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(380.0),
                        height: Dimension::Auto,
                    },
                    ..column(theme.spacing_large)
                },
                theme.spacing_large,
            );
            let mut content = RawView::new(column(theme.spacing_small))
                .child(Box::new(Heading::md(title)))
                .child(Box::new(
                    Text::secondary(message).text_align(TextAlign::Start),
                ));
            let mut actions = RawView::new(Style {
                justify_content: Some(JustifyContent::End),
                ..row(theme.spacing_medium)
            });
            for (label, variant, action) in &actions_data {
                let action = action.clone();
                actions = actions.child(Box::new(Button::styled(
                    *variant,
                    ButtonSize::Md,
                    label.clone(),
                    ButtonState::Normal,
                    move || action(),
                )));
            }
            content = content.child(Box::new(actions));
            vec![Box::new(Popover::new(card_style).child(Box::new(content))) as BoxedWidget]
        }
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        Some(self.dismiss.clone())
    }
}

/// Convenience dialog for a short message and one or two explicit actions.
pub struct AlertDialog {
    inner: Dialog,
}

impl AlertDialog {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        on_dismiss: impl Fn() + 'static,
    ) -> Self {
        Self {
            inner: Dialog::new(title, message, on_dismiss),
        }
    }
    pub fn confirm(mut self, label: impl Into<String>, on_confirm: impl Fn() + 'static) -> Self {
        self.inner = self.inner.action(label, ButtonVariant::Primary, on_confirm);
        self
    }
    pub fn dismiss_button(mut self, label: impl Into<String>) -> Self {
        self.inner = self.inner.dismiss_action(label);
        self
    }
}

impl Widget for AlertDialog {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect)
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }
}

/// A horizontal determinate or indeterminate progress indicator.
pub struct ProgressBar {
    theme: Theme,
    value: Option<f32>,
    style: creamui_core::Style,
}
impl_styled_field!(ProgressBar);

impl ProgressBar {
    pub fn new(value: f32) -> Self {
        let theme = use_theme();
        Self {
            theme,
            value: Some(value),
            style: Self::default_style().into(),
        }
    }
    pub fn indeterminate() -> Self {
        let theme = use_theme();
        Self {
            theme,
            value: None,
            style: Self::default_style().into(),
        }
    }
    pub fn default_style() -> Style {
        Style {
            size: fixed(200.0, 10.0),
            ..Default::default()
        }
    }
}

impl Widget for ProgressBar {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let radius = rect.height / 2.0;
        painter.fill_rect(rect, self.theme.surface_hover, radius);
        let (x, width) = match self.value {
            Some(value) => (rect.x, rect.width * value.clamp(0.0, 1.0)),
            None => {
                let width = rect.width * 0.32;
                let travel = rect.width + width;
                let phase = (painter.animation_time() * 0.8).fract();
                (rect.x - width + travel * phase, width)
            }
        };
        if width > 0.0 {
            painter.push_clip_rounded(rect, radius);
            painter.fill_rect(
                Rect {
                    x,
                    y: rect.y,
                    width,
                    height: rect.height,
                },
                self.theme.accent,
                radius,
            );
            painter.pop_clip();
        }
    }
}

/// A circular determinate or indeterminate progress indicator.
pub struct ProgressRing {
    theme: Theme,
    value: Option<f32>,
    size: f32,
}

impl ProgressRing {
    pub fn new(value: f32) -> Self {
        let theme = use_theme();
        Self {
            theme,
            value: Some(value),
            size: 28.0,
        }
    }
    pub fn indeterminate() -> Self {
        let theme = use_theme();
        Self {
            theme,
            value: None,
            size: 28.0,
        }
    }
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

impl Widget for ProgressRing {
    fn style(&self) -> creamui_core::Style {
        Style {
            size: fixed(self.size, self.size),
            ..Default::default()
        }
        .into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let center = Point {
            x: rect.x + rect.width / 2.0,
            y: rect.y + rect.height / 2.0,
        };
        let radius = rect.width.min(rect.height) * 0.38;
        let stroke = (self.size * 0.12).max(2.0);
        let draw_arc = |p: &mut dyn Painter, start: f32, amount: f32, color: Color| {
            let steps = 32;
            let point = |t: f32| Point {
                x: center.x + radius * t.cos(),
                y: center.y + radius * t.sin(),
            };
            for index in 0..steps {
                let a = start + amount * index as f32 / steps as f32;
                let b = start + amount * (index + 1) as f32 / steps as f32;
                p.stroke_line(point(a), point(b), color, stroke);
            }
        };
        draw_arc(
            painter,
            0.0,
            std::f32::consts::TAU,
            self.theme.surface_hover,
        );
        let (start, amount) = match self.value {
            Some(value) => (
                -std::f32::consts::FRAC_PI_2,
                std::f32::consts::TAU * value.clamp(0.0, 1.0),
            ),
            None => (
                painter.animation_time() * std::f32::consts::TAU,
                std::f32::consts::TAU * 0.28,
            ),
        };
        draw_arc(painter, start, amount, self.theme.accent);
    }
}

/// A small pill or dot indicator, e.g. an unread-message count or an
/// online-status marker. Laid out as a normal flex item (not an overlay) —
/// place it where it belongs in the row/column and it takes up exactly the
/// room it needs.
pub struct Badge {
    style: Style,
    background: Color,
    text_color: Color,
    label: Option<String>,
}

impl Badge {
    fn base_style(width: f32, height: f32) -> Style {
        Style {
            size: fixed(width, height),
            flex_shrink: 0.,
            ..Default::default()
        }
    }

    /// A numeric badge, e.g. an unread-message count. `count == 0` collapses
    /// to zero size, so callers can include it unconditionally instead of
    /// branching it out of the layout by hand.
    pub fn count(count: usize) -> Self {
        let theme = use_theme();
        let label = match count {
            0 => None,
            1..=99 => Some(count.to_string()),
            _ => Some("99+".to_owned()),
        };
        let height: f32 = 18.0;
        let width = match &label {
            None => 0.0,
            Some(text) => height.max(11.0 + text.len() as f32 * 7.5),
        };
        Self {
            style: Self::base_style(width, height),
            background: theme.accent,
            text_color: theme.selection_text,
            label,
        }
    }

    /// A plain colored dot with no label, e.g. an online-status marker used
    /// inline rather than overlaid on an avatar.
    pub fn dot(color: Color, size: f32) -> Self {
        Self {
            style: Self::base_style(size, size),
            background: color,
            text_color: color,
            label: None,
        }
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }
}

impl Widget for Badge {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }
        painter.fill_rect(rect, self.background, rect.height / 2.0);
        if let Some(label) = &self.label {
            painter.fill_text_weight(
                rect,
                label,
                self.text_color,
                (rect.height * 0.6).max(9.0),
                TextAlign::Center,
                true,
                false,
            );
        }
    }
}

/// Three dots that bounce in sequence while composing is in progress, driven
/// by [`Painter::animation_time`] — the same "call it every paint, and the
/// window keeps requesting new frames while it's visible" pattern
/// [`ProgressRing::indeterminate`] uses. A common "the other party is typing"
/// affordance for chat-shaped UIs.
pub struct TypingIndicator {
    theme: Theme,
    style: creamui_core::Style,
}
impl_styled_field!(TypingIndicator);

impl TypingIndicator {
    pub fn new() -> Self {
        let theme = use_theme();
        Self {
            theme,
            style: Style {
                size: fixed(36.0, 16.0),
                flex_shrink: 0.,
                ..Default::default()
            }
            .into(),
        }
    }
}

impl Widget for TypingIndicator {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let time = painter.animation_time();
        let dot = (rect.height * 0.5).max(3.0);
        let gap = ((rect.width - dot * 3.0) / 2.0).max(2.0);
        for i in 0..3 {
            let phase = (time * 2.6 - i as f32 * 0.3).rem_euclid(1.8);
            let lift = if phase < 0.6 {
                (phase * std::f32::consts::PI / 0.6).sin()
            } else {
                0.0
            };
            let cx = rect.x + dot / 2.0 + i as f32 * (dot + gap);
            let cy = rect.y + rect.height / 2.0 - lift * rect.height * 0.3;
            painter.fill_rect(
                Rect {
                    x: cx - dot / 2.0,
                    y: cy - dot / 2.0,
                    width: dot,
                    height: dot,
                },
                self.theme.text_secondary,
                dot / 2.0,
            );
        }
    }
}
