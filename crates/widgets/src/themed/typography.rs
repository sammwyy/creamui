use super::*;
use crate::raw::{RawLink, RawPre, RawQuote};

/// A themed blockquote: an accent-colored bar along the left edge of an
/// inset, italicized line — a [`RawQuote`] wrapping one [`RawText`] child.
pub struct Quote {
    inner: RawQuote,
}
impl_styled_inner!(Quote);

impl Quote {
    pub fn new(text: impl Into<String>) -> Self {
        let theme = use_theme();
        let bar_width = 3.0;
        let style = Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            padding: LayoutRect {
                left: LengthPercentage::Length(bar_width + theme.spacing_medium),
                right: LengthPercentage::Length(theme.spacing_small),
                top: LengthPercentage::Length(theme.spacing_small),
                bottom: LengthPercentage::Length(theme.spacing_small),
            },
            ..Default::default()
        };
        let content = RawText::new(text, theme.text_secondary, theme.typography.body)
            .italic(true)
            .text_align(TextAlign::Start);
        Quote {
            inner: RawQuote::new(style, theme.accent, bar_width).child(Box::new(content)),
        }
    }
}

impl Widget for Quote {
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

/// A themed preformatted / code block: text on its own inset surface,
/// whitespace and line breaks preserved. A monospace face is not bundled, so
/// column alignment is only approximate unless the application selects one.
pub struct Pre {
    inner: RawPre,
}
impl_styled_inner!(Pre);

impl Pre {
    pub fn new(text: impl Into<String>) -> Self {
        let theme = use_theme();
        let style = Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            ..Default::default()
        };
        Pre {
            inner: RawPre::new(style, text, theme.text_primary, theme.typography.caption)
                .font_family(theme.font_family)
                .background(theme.surface_elevated)
                .corner_radius(theme.radius_medium)
                .padding(theme.spacing_medium),
        }
    }
}

impl Widget for Pre {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn measure(&self) -> Option<creamui_core::MeasureFn> {
        self.inner.measure()
    }
}

/// A themed hyperlink: accent-colored, underlined text that brightens on
/// hover — a [`RawLink`] with the theme's accent/hover colors filled in.
pub struct Link {
    inner: RawLink,
}
impl_styled_inner!(Link);

impl Link {
    pub fn new(text: impl Into<String>, on_click: impl Fn() + 'static) -> Self {
        let theme = use_theme();
        let style = Style {
            size: creamui_core::layout::Size {
                width: Dimension::Auto,
                height: Dimension::Length(theme.typography.body * 1.4),
            },
            ..Default::default()
        };
        Link {
            inner: RawLink::new(style, text, theme.accent, theme.typography.body, on_click)
                .font_family(theme.font_family)
                .hover_color(theme.accent_hover),
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.inner.disabled = disabled;
        self
    }
}

impl Widget for Link {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn measure(&self) -> Option<creamui_core::MeasureFn> {
        self.inner.measure()
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
