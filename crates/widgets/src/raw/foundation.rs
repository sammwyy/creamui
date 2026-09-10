use super::*;
/// A controlled text selection represented as byte offsets into a UTF-8
/// document. `anchor` stays at the point where selection began while `focus`
/// follows the caret or pointer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextSelection {
    pub anchor: usize,
    pub focus: usize,
}

impl TextSelection {
    pub fn range(self) -> std::ops::Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }

    pub fn is_empty(self) -> bool {
        self.anchor == self.focus
    }
}

/// An unstyled rectangular container that lays out its children.
pub struct RawView {
    pub style: creamui_core::Style,
    pub children: Vec<BoxedWidget>,
}

impl RawView {
    pub fn new(style: impl Into<creamui_core::Style>) -> Self {
        let style = style.into();
        RawView {
            style,
            children: Vec::new(),
        }
    }

    pub fn child(mut self, widget: BoxedWidget) -> Self {
        self.children.push(widget);
        self
    }

    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.children = widgets;
        self
    }
}

impl Widget for RawView {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }
}

/// Unstyled text with no color or size opinion beyond what's passed in.
pub struct RawText {
    pub text: String,
    pub style: creamui_core::Style,
}

impl RawText {
    /// Creates text with semantic defaults, ready to receive a shared style.
    pub fn unstyled(text: impl Into<String>) -> Self {
        RawText {
            text: text.into(),
            style: creamui_core::Style::new().text_align(TextAlign::Center),
        }
    }

    pub fn new(text: impl Into<String>, color: Color, font_size: f32) -> Self {
        Self::unstyled(text).color(color).font_size(font_size)
    }
}

impl Widget for RawText {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let state = creamui_core::StyleState::NORMAL
            .with_hovered(painter.hovered(rect))
            .with_pressed(painter.pressed(rect));
        let typography = self.style.resolve(state).typography;
        let colors = painter.color_scheme();
        let color = typography
            .color
            .unwrap_or_else(|| Color::rgb(0, 0, 0).into())
            .resolve(&colors);
        let font_size = typography.font_size.unwrap_or(14.0);
        let align = typography.align.unwrap_or_default();
        let bold = typography.bold.unwrap_or(false);
        let italic = typography.italic.unwrap_or(false);
        let underline = typography.underline.unwrap_or(false);
        let strikethrough = typography.strikethrough.unwrap_or(false);
        let family = typography.font_family.as_deref();
        painter.fill_text_font(
            rect, &self.text, color, font_size, align, family, bold, italic,
        );
        super::draw_text_decorations(
            painter,
            rect,
            &self.text,
            font_size,
            family,
            bold,
            align,
            color,
            underline,
            strikethrough,
        );
    }

    fn measure(&self) -> Option<creamui_core::MeasureFn> {
        let text = self.text.clone();
        let typography = self
            .style
            .resolve(creamui_core::StyleState::NORMAL)
            .typography;
        let font_size = typography.font_size.unwrap_or(14.0);
        let bold = typography.bold.unwrap_or(false);
        let family = typography.font_family;
        Some(Box::new(move |known_dimensions, available_space| {
            let max_width = match (known_dimensions.width, available_space.width) {
                (Some(w), _) => w,
                (None, creamui_core::layout::AvailableSpace::Definite(w)) => w,
                (None, _) => crate::text_metrics::unbounded_width(),
            };
            let (natural_width, natural_height) = crate::text_metrics::measure_family(
                &text,
                font_size,
                max_width,
                family.as_deref(),
                bold,
            );
            creamui_core::layout::Size {
                width: known_dimensions.width.unwrap_or(natural_width),
                height: known_dimensions.height.unwrap_or(natural_height),
            }
        }))
    }
}
