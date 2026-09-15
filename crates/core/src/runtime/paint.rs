use std::rc::Rc;

use super::node::{NodeKind, RuntimeNode};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadPrimitive {
    pub rect: crate::Rect,
    pub color: creamui_theme::Color,
    pub corner_radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderPrimitive {
    pub rect: crate::Rect,
    pub color: creamui_theme::Color,
    pub width: f32,
    pub corner_radius: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextPrimitive {
    pub rect: crate::Rect,
    pub text: Rc<str>,
    pub color: creamui_theme::Color,
    pub font_size: f32,
    pub align: crate::TextAlign,
    pub family: Option<Rc<str>>,
    pub bold: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImagePrimitive {
    pub rect: crate::Rect,
    pub source: Rc<str>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaintPrimitive {
    Quad(QuadPrimitive),
    Border(BorderPrimitive),
    Text(TextPrimitive),
    Image(ImagePrimitive),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaintOp {
    PushClip(crate::Rect),
    PopClip,
    PushTransform(super::mutation::Transform2D),
    PopTransform,
    Primitive(PaintPrimitive),
}

/// One node's retained paint output — immutable until regenerated.
#[derive(Default)]
pub struct PaintFragment {
    pub ops: Vec<PaintOp>,
    pub bounds: crate::Rect,
}

#[derive(Default)]
pub struct PaintState {
    pub fragment: Option<PaintFragment>,
}

/// Regenerates `node`'s fragment from its own style/content. `Custom`
/// nodes (legacy-mounted `Widget`s) produce an empty fragment — the
/// widget that could paint them is dropped after
/// [`super::mount::mount_legacy_widget`] runs.
pub(super) fn generate_fragment(
    node: &RuntimeNode,
    colors: &creamui_theme::ColorScheme,
) -> PaintFragment {
    let rect = node.layout.rect;
    let mut ops = Vec::new();
    let paint = &node.paint_style;

    if let Some(background) = paint.background {
        ops.push(PaintOp::Primitive(PaintPrimitive::Quad(QuadPrimitive {
            rect,
            color: background.resolve(colors),
            corner_radius: paint.corner_radius.unwrap_or(0.0),
        })));
    }

    match &node.kind {
        NodeKind::Text(text) => {
            let typography = &node.typography_style;
            ops.push(PaintOp::Primitive(PaintPrimitive::Text(TextPrimitive {
                rect,
                text: text.text.clone(),
                color: typography
                    .color
                    .map(|c| c.resolve(colors))
                    .unwrap_or(creamui_theme::Color::rgb(0, 0, 0)),
                font_size: typography.font_size.unwrap_or(14.0),
                align: typography.align.unwrap_or_default(),
                family: typography.font_family.as_deref().map(Rc::from),
                bold: typography.bold.unwrap_or(false),
                underline: typography.underline.unwrap_or(false),
                strikethrough: typography.strikethrough.unwrap_or(false),
            })));
        }
        NodeKind::Image(image) => {
            ops.push(PaintOp::Primitive(PaintPrimitive::Image(ImagePrimitive {
                rect,
                source: image.source.clone(),
            })));
        }
        NodeKind::Container | NodeKind::Custom(_) => {}
    }

    if let Some(border) = paint.border {
        ops.push(PaintOp::Primitive(PaintPrimitive::Border(
            BorderPrimitive {
                rect,
                color: border.color.resolve(colors),
                width: border.width,
                corner_radius: paint.corner_radius.unwrap_or(0.0),
            },
        )));
    }

    PaintFragment { ops, bounds: rect }
}

/// Records a legacy `Widget::paint` call as [`PaintOp`]s instead of
/// rasterizing it — the migration path REFACTOR.md 13.2 describes: an
/// existing widget's painter can produce retained primitives without
/// itself changing.
pub struct RecordingPainter {
    color_scheme: creamui_theme::ColorScheme,
    ops: Vec<PaintOp>,
}

impl RecordingPainter {
    pub fn new(color_scheme: creamui_theme::ColorScheme) -> Self {
        RecordingPainter {
            color_scheme,
            ops: Vec::new(),
        }
    }

    pub fn into_ops(self) -> Vec<PaintOp> {
        self.ops
    }

    #[allow(clippy::too_many_arguments)]
    fn push_text(
        &mut self,
        rect: crate::Rect,
        text: &str,
        color: creamui_theme::Color,
        font_size: f32,
        align: crate::TextAlign,
        family: Option<&str>,
        bold: bool,
    ) {
        self.ops
            .push(PaintOp::Primitive(PaintPrimitive::Text(TextPrimitive {
                rect,
                text: Rc::from(text),
                color,
                font_size,
                align,
                family: family.map(Rc::from),
                bold,
                underline: false,
                strikethrough: false,
            })));
    }
}

impl crate::Painter for RecordingPainter {
    fn color_scheme(&self) -> creamui_theme::ColorScheme {
        self.color_scheme
    }

    fn fill_rect(&mut self, rect: crate::Rect, color: creamui_theme::Color, corner_radius: f32) {
        self.ops
            .push(PaintOp::Primitive(PaintPrimitive::Quad(QuadPrimitive {
                rect,
                color,
                corner_radius,
            })));
    }

    fn stroke_rect(
        &mut self,
        rect: crate::Rect,
        color: creamui_theme::Color,
        width: f32,
        corner_radius: f32,
    ) {
        self.ops.push(PaintOp::Primitive(PaintPrimitive::Border(
            BorderPrimitive {
                rect,
                color,
                width,
                corner_radius,
            },
        )));
    }

    fn fill_text(
        &mut self,
        rect: crate::Rect,
        text: &str,
        color: creamui_theme::Color,
        font_size: f32,
        align: crate::TextAlign,
    ) {
        self.push_text(rect, text, color, font_size, align, None, false);
    }

    fn fill_text_weight(
        &mut self,
        rect: crate::Rect,
        text: &str,
        color: creamui_theme::Color,
        font_size: f32,
        align: crate::TextAlign,
        bold: bool,
        _italic: bool,
    ) {
        self.push_text(rect, text, color, font_size, align, None, bold);
    }

    fn fill_text_font(
        &mut self,
        rect: crate::Rect,
        text: &str,
        color: creamui_theme::Color,
        font_size: f32,
        align: crate::TextAlign,
        family: Option<&str>,
        bold: bool,
        _italic: bool,
    ) {
        self.push_text(rect, text, color, font_size, align, family, bold);
    }

    fn push_clip(&mut self, rect: crate::Rect) {
        self.ops.push(PaintOp::PushClip(rect));
    }

    fn pop_clip(&mut self) {
        self.ops.push(PaintOp::PopClip);
    }
}

#[cfg(test)]
mod recording_painter_tests {
    use super::*;
    use crate::{Painter, Rect, Widget};

    struct Card;
    impl crate::Widget for Card {
        fn style(&self) -> crate::Style {
            crate::Style::default()
        }
        fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
            painter.push_clip(rect);
            painter.fill_rect(rect, creamui_theme::Color::rgb(1, 2, 3), 4.0);
            painter.pop_clip();
        }
    }

    #[test]
    fn records_a_widgets_paint_calls_as_ops() {
        let mut painter = RecordingPainter::new(creamui_theme::ColorScheme::default());
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        Card.paint(&mut painter, rect);

        assert_eq!(
            painter.into_ops(),
            vec![
                PaintOp::PushClip(rect),
                PaintOp::Primitive(PaintPrimitive::Quad(QuadPrimitive {
                    rect,
                    color: creamui_theme::Color::rgb(1, 2, 3),
                    corner_radius: 4.0,
                })),
                PaintOp::PopClip,
            ]
        );
    }

    #[test]
    fn fill_text_weight_records_bold_but_not_family() {
        let mut painter = RecordingPainter::new(creamui_theme::ColorScheme::default());
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        painter.fill_text_weight(
            rect,
            "hi",
            creamui_theme::Color::rgb(1, 2, 3),
            14.0,
            crate::TextAlign::Start,
            true,
            false,
        );

        assert_eq!(
            painter.into_ops(),
            vec![PaintOp::Primitive(PaintPrimitive::Text(TextPrimitive {
                rect,
                text: Rc::from("hi"),
                color: creamui_theme::Color::rgb(1, 2, 3),
                font_size: 14.0,
                align: crate::TextAlign::Start,
                family: None,
                bold: true,
                underline: false,
                strikethrough: false,
            }))]
        );
    }

    #[test]
    fn fill_text_font_records_family_and_bold() {
        let mut painter = RecordingPainter::new(creamui_theme::ColorScheme::default());
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        painter.fill_text_font(
            rect,
            "hi",
            creamui_theme::Color::rgb(1, 2, 3),
            14.0,
            crate::TextAlign::Start,
            Some("Inter"),
            true,
            false,
        );

        assert_eq!(
            painter.into_ops(),
            vec![PaintOp::Primitive(PaintPrimitive::Text(TextPrimitive {
                rect,
                text: Rc::from("hi"),
                color: creamui_theme::Color::rgb(1, 2, 3),
                font_size: 14.0,
                align: crate::TextAlign::Start,
                family: Some(Rc::from("Inter")),
                bold: true,
                underline: false,
                strikethrough: false,
            }))]
        );
    }
}
