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

    fn clips_children(&self) -> bool {
        true
    }

    fn clip_corner_radius(&self) -> f32 {
        self.style.paint.corner_radius.unwrap_or(0.0)
    }
}

pub struct RawTranslate {
    pub style: creamui_core::Style,
    pub children: Vec<BoxedWidget>,
    pub translation: Rc<Cell<Point>>,
}

impl RawTranslate {
    pub fn new(style: impl Into<creamui_core::Style>, translation: Rc<Cell<Point>>) -> Self {
        Self {
            style: style.into(),
            children: Vec::new(),
            translation,
        }
    }

    pub fn child(mut self, widget: BoxedWidget) -> Self {
        self.children.push(widget);
        self
    }
}

impl Widget for RawTranslate {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }

    fn scroll_offset(&self) -> Point {
        let translation = self.translation.get();
        Point {
            x: -translation.x,
            y: -translation.y,
        }
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

    fn legacy_node_kind(&self) -> Option<creamui_core::runtime::NodeKind> {
        Some(creamui_core::runtime::NodeKind::Text(
            creamui_core::runtime::TextNode {
                text: self.text.as_str().into(),
            },
        ))
    }

    fn measure_fingerprint(&self) -> Option<u64> {
        use std::hash::{Hash, Hasher};
        // Must cover exactly what `measure`'s closure captures below — its
        // width/height inputs come from `taffy` at call time, not from
        // here, so this fingerprint doesn't need to (and can't) account
        // for them.
        let typography = self
            .style
            .resolve(creamui_core::StyleState::NORMAL)
            .typography;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.text.hash(&mut hasher);
        typography
            .font_size
            .unwrap_or(14.0)
            .to_bits()
            .hash(&mut hasher);
        typography.bold.unwrap_or(false).hash(&mut hasher);
        typography.font_family.hash(&mut hasher);
        Some(hasher.finish())
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

#[cfg(test)]
mod raw_text_fingerprint_tests {
    use super::*;

    #[test]
    fn measure_fingerprint_ignores_color_but_not_text_or_size() {
        let a = RawText::new("hi", Color::rgb(1, 2, 3), 14.0);
        let b = RawText::new("hi", Color::rgb(4, 5, 6), 14.0);
        assert_eq!(
            a.measure_fingerprint(),
            b.measure_fingerprint(),
            "color does not affect measurement"
        );

        let c = RawText::new("bye", Color::rgb(1, 2, 3), 14.0);
        assert_ne!(a.measure_fingerprint(), c.measure_fingerprint());

        let d = RawText::new("hi", Color::rgb(1, 2, 3), 20.0);
        assert_ne!(a.measure_fingerprint(), d.measure_fingerprint());
    }

    #[test]
    fn legacy_node_kind_reports_its_text_content() {
        let text = RawText::new("hello", Color::rgb(1, 2, 3), 14.0);
        match text.legacy_node_kind() {
            Some(creamui_core::runtime::NodeKind::Text(node)) => {
                assert_eq!(&*node.text, "hello")
            }
            other => panic!("expected a text node kind, got {other:?}"),
        }
    }

    #[test]
    fn mounting_a_raw_text_produces_a_runtime_text_node() {
        use creamui_core::runtime::{mount_legacy_widget, NodeKind, Runtime};

        let widget: creamui_core::BoxedWidget =
            Box::new(RawText::new("hello", Color::rgb(1, 2, 3), 14.0));
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let root = mount_legacy_widget(&mut tx, widget, None);
        drop(tx);

        match &runtime.get(root).unwrap().kind {
            NodeKind::Text(node) => assert_eq!(&*node.text, "hello"),
            other => panic!("expected a text node kind, got {other:?}"),
        }
    }
}
