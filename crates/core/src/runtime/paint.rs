use std::rc::Rc;

use super::node::{ImageContent, ImageFit, NodeKind, RuntimeNode};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadPrimitive {
    pub rect: crate::Rect,
    pub color: creamui_theme::Color,
    pub corner_radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientPrimitive {
    pub rect: crate::Rect,
    pub start: creamui_theme::Color,
    pub end: creamui_theme::Color,
    pub angle_degrees: f32,
    pub corner_radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadialGradientPrimitive {
    pub rect: crate::Rect,
    pub start: creamui_theme::Color,
    pub end: creamui_theme::Color,
    pub center: crate::Point,
    pub radius: f32,
    pub radius_y: f32,
    pub repeating: bool,
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
    pub italic: bool,
    pub selection: Option<(std::ops::Range<usize>, creamui_theme::Color)>,
    pub underline: bool,
    pub strikethrough: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImagePrimitive {
    pub rect: crate::Rect,
    pub content: ImageContent,
    pub tint: Option<creamui_theme::Color>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaintPrimitive {
    Quad(QuadPrimitive),
    Gradient(GradientPrimitive),
    RadialGradient(RadialGradientPrimitive),
    Border(BorderPrimitive),
    Text(TextPrimitive),
    Image(ImagePrimitive),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaintOp {
    PushClip(crate::Rect),
    PushRoundedClip(crate::Rect, f32),
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

fn translated(rect: crate::Rect, x: f32, y: f32) -> crate::Rect {
    crate::Rect {
        x: rect.x + x,
        y: rect.y + y,
        ..rect
    }
}

fn faded(color: creamui_theme::Color, opacity: f32) -> creamui_theme::Color {
    creamui_theme::Color::rgba(
        color.r,
        color.g,
        color.b,
        (color.a as f32 * opacity).round() as u8,
    )
}

pub(super) fn paint_fragment(
    fragment: &PaintFragment,
    painter: &mut dyn crate::Painter,
    transform: super::mutation::Transform2D,
    opacity: f32,
) {
    let mut offsets = Vec::new();
    let mut offset = (transform.x, transform.y);
    let mut clips = 0;
    for op in &fragment.ops {
        let (x, y) = offset;
        match op {
            PaintOp::PushClip(rect) => {
                painter.push_clip(translated(*rect, x, y));
                clips += 1;
            }
            PaintOp::PushRoundedClip(rect, radius) => {
                painter.push_clip_rounded(translated(*rect, x, y), *radius);
                clips += 1;
            }
            PaintOp::PopClip => {
                painter.pop_clip();
                clips -= 1;
            }
            PaintOp::PushTransform(next) => {
                offsets.push(offset);
                offset = (x + next.x, y + next.y);
            }
            PaintOp::PopTransform => {
                offset = offsets.pop().expect("transform push precedes pop");
            }
            PaintOp::Primitive(primitive) => match primitive {
                PaintPrimitive::Quad(quad) => painter.fill_rect(
                    translated(quad.rect, x, y),
                    faded(quad.color, opacity),
                    quad.corner_radius,
                ),
                PaintPrimitive::Gradient(gradient) => painter.fill_linear_gradient(
                    translated(gradient.rect, x, y),
                    faded(gradient.start, opacity),
                    faded(gradient.end, opacity),
                    gradient.angle_degrees,
                    gradient.corner_radius,
                ),
                PaintPrimitive::RadialGradient(gradient) => painter
                    .fill_radial_gradient_ellipse_repeating(
                        translated(gradient.rect, x, y),
                        faded(gradient.start, opacity),
                        faded(gradient.end, opacity),
                        crate::Point {
                            x: gradient.center.x + x,
                            y: gradient.center.y + y,
                        },
                        [gradient.radius, gradient.radius_y],
                        gradient.repeating,
                        gradient.corner_radius,
                    ),
                PaintPrimitive::Border(border) => painter.stroke_rect(
                    translated(border.rect, x, y),
                    faded(border.color, opacity),
                    border.width,
                    border.corner_radius,
                ),
                PaintPrimitive::Text(text) => {
                    let rect = translated(text.rect, x, y);
                    let color = faded(text.color, opacity);
                    if let Some((selection, selected_color)) = &text.selection {
                        painter.fill_text_selected_weight_font(
                            rect,
                            &text.text,
                            color,
                            faded(*selected_color, opacity),
                            selection.clone(),
                            text.font_size,
                            text.align,
                            text.family.as_deref(),
                            text.bold,
                            text.italic,
                        );
                    } else {
                        painter.fill_text_font(
                            rect,
                            &text.text,
                            color,
                            text.font_size,
                            text.align,
                            text.family.as_deref(),
                            text.bold,
                            text.italic,
                        );
                    }
                    if text.underline || text.strikethrough {
                        painter.draw_text_decorations(
                            rect,
                            &text.text,
                            color,
                            text.font_size,
                            text.align,
                            text.family.as_deref(),
                            text.bold,
                            text.underline,
                            text.strikethrough,
                        );
                    }
                }
                PaintPrimitive::Image(image) => {
                    if let ImageContent::Decoded(decoded) = &image.content {
                        painter.draw_image_opacity(
                            translated(image.rect, x, y),
                            decoded,
                            image.tint,
                            opacity,
                        );
                    }
                }
            },
        }
    }
    debug_assert_eq!(clips, 0);
    debug_assert!(offsets.is_empty());
}

fn image_destination(rect: crate::Rect, content: &ImageContent, fit: ImageFit) -> crate::Rect {
    let ImageContent::Decoded(image) = content else {
        return rect;
    };
    if fit == ImageFit::Fill {
        return rect;
    }
    let width = image.width() as f32;
    let height = image.height() as f32;
    if fit == ImageFit::None {
        return crate::Rect {
            x: rect.x,
            y: rect.y,
            width,
            height,
        };
    }
    let scale_x = rect.width / width;
    let scale_y = rect.height / height;
    let scale = if fit == ImageFit::Contain {
        scale_x.min(scale_y)
    } else {
        scale_x.max(scale_y)
    };
    let width = width * scale;
    let height = height * scale;
    crate::Rect {
        x: rect.x + (rect.width - width) / 2.0,
        y: rect.y + (rect.height - height) / 2.0,
        width,
        height,
    }
}

/// Regenerates `node`'s fragment from its own style/content.
pub(super) fn generate_fragment(
    node: &RuntimeNode,
    colors: &creamui_theme::ColorScheme,
) -> PaintFragment {
    let rect = node.layout.rect;
    let mut ops = Vec::new();
    let paint = &node.paint_style;

    if let Some(background) = paint.background {
        let primitive = match background {
            crate::Background::Solid(color) => PaintPrimitive::Quad(QuadPrimitive {
                rect,
                color: color.resolve(colors),
                corner_radius: paint.corner_radius.unwrap_or(0.0),
            }),
            crate::Background::LinearGradient(gradient) => {
                PaintPrimitive::Gradient(GradientPrimitive {
                    rect,
                    start: gradient.start.resolve(colors),
                    end: gradient.end.resolve(colors),
                    angle_degrees: gradient.angle_degrees,
                    corner_radius: paint.corner_radius.unwrap_or(0.0),
                })
            }
            crate::Background::RadialGradient(gradient) => {
                let (center, radii) = gradient.geometry(rect);
                PaintPrimitive::RadialGradient(RadialGradientPrimitive {
                    rect,
                    start: gradient.start.resolve(colors),
                    end: gradient.end.resolve(colors),
                    center,
                    radius: radii[0],
                    radius_y: radii[1],
                    repeating: gradient.repeating,
                    corner_radius: paint.corner_radius.unwrap_or(0.0),
                })
            }
        };
        ops.push(PaintOp::Primitive(primitive));
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
                italic: typography.italic.unwrap_or(false),
                selection: None,
                underline: typography.underline.unwrap_or(false),
                strikethrough: typography.strikethrough.unwrap_or(false),
            })));
        }
        NodeKind::Image(image) => {
            let radius = paint.corner_radius.unwrap_or(0.0);
            let clipped = image.fit == ImageFit::Cover || radius > 0.0;
            if clipped {
                if radius > 0.0 {
                    ops.push(PaintOp::PushRoundedClip(rect, radius));
                } else {
                    ops.push(PaintOp::PushClip(rect));
                }
            }
            ops.push(PaintOp::Primitive(PaintPrimitive::Image(ImagePrimitive {
                rect: image_destination(rect, &image.content, image.fit),
                content: image.content.clone(),
                tint: None,
            })));
            if clipped {
                ops.push(PaintOp::PopClip);
            }
        }
        NodeKind::Custom(custom) => {
            if let Some(widget) = &custom.widget {
                let mut recorder = RecordingPainter::new(*colors);
                widget.paint_content(&mut recorder, rect, node.layout.content_rect);
                ops.extend(recorder.into_ops());
            }
        }
        NodeKind::Container => {}
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
        italic: bool,
        selection: Option<(std::ops::Range<usize>, creamui_theme::Color)>,
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
                italic,
                selection,
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

    fn fill_linear_gradient(
        &mut self,
        rect: crate::Rect,
        start: creamui_theme::Color,
        end: creamui_theme::Color,
        angle_degrees: f32,
        corner_radius: f32,
    ) {
        self.ops.push(PaintOp::Primitive(PaintPrimitive::Gradient(
            GradientPrimitive {
                rect,
                start,
                end,
                angle_degrees,
                corner_radius,
            },
        )));
    }

    fn fill_radial_gradient(
        &mut self,
        rect: crate::Rect,
        start: creamui_theme::Color,
        end: creamui_theme::Color,
        center: crate::Point,
        radius: f32,
        corner_radius: f32,
    ) {
        self.fill_radial_gradient_ellipse(rect, start, end, center, [radius; 2], corner_radius);
    }

    fn fill_radial_gradient_ellipse(
        &mut self,
        rect: crate::Rect,
        start: creamui_theme::Color,
        end: creamui_theme::Color,
        center: crate::Point,
        radii: [f32; 2],
        corner_radius: f32,
    ) {
        self.fill_radial_gradient_ellipse_repeating(
            rect,
            start,
            end,
            center,
            radii,
            false,
            corner_radius,
        );
    }

    fn fill_radial_gradient_ellipse_repeating(
        &mut self,
        rect: crate::Rect,
        start: creamui_theme::Color,
        end: creamui_theme::Color,
        center: crate::Point,
        radii: [f32; 2],
        repeating: bool,
        corner_radius: f32,
    ) {
        self.ops
            .push(PaintOp::Primitive(PaintPrimitive::RadialGradient(
                RadialGradientPrimitive {
                    rect,
                    start,
                    end,
                    center,
                    radius: radii[0],
                    radius_y: radii[1],
                    repeating,
                    corner_radius,
                },
            )));
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
        self.push_text(
            rect, text, color, font_size, align, None, false, false, None,
        );
    }

    fn fill_text_weight(
        &mut self,
        rect: crate::Rect,
        text: &str,
        color: creamui_theme::Color,
        font_size: f32,
        align: crate::TextAlign,
        bold: bool,
        italic: bool,
    ) {
        self.push_text(
            rect, text, color, font_size, align, None, bold, italic, None,
        );
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
        italic: bool,
    ) {
        self.push_text(
            rect, text, color, font_size, align, family, bold, italic, None,
        );
    }

    fn fill_text_selected(
        &mut self,
        rect: crate::Rect,
        text: &str,
        color: creamui_theme::Color,
        selected_color: creamui_theme::Color,
        selected: std::ops::Range<usize>,
        font_size: f32,
        align: crate::TextAlign,
    ) {
        self.push_text(
            rect,
            text,
            color,
            font_size,
            align,
            None,
            false,
            false,
            Some((selected, selected_color)),
        );
    }

    fn fill_text_selected_font(
        &mut self,
        rect: crate::Rect,
        text: &str,
        color: creamui_theme::Color,
        selected_color: creamui_theme::Color,
        selected: std::ops::Range<usize>,
        font_size: f32,
        align: crate::TextAlign,
        family: Option<&str>,
    ) {
        self.push_text(
            rect,
            text,
            color,
            font_size,
            align,
            family,
            false,
            false,
            Some((selected, selected_color)),
        );
    }

    fn fill_text_selected_weight_font(
        &mut self,
        rect: crate::Rect,
        text: &str,
        color: creamui_theme::Color,
        selected_color: creamui_theme::Color,
        selected: std::ops::Range<usize>,
        font_size: f32,
        align: crate::TextAlign,
        family: Option<&str>,
        bold: bool,
        italic: bool,
    ) {
        self.push_text(
            rect,
            text,
            color,
            font_size,
            align,
            family,
            bold,
            italic,
            Some((selected, selected_color)),
        );
    }

    fn draw_image(
        &mut self,
        rect: crate::Rect,
        image: &crate::RgbaImage,
        tint: Option<creamui_theme::Color>,
    ) {
        self.ops
            .push(PaintOp::Primitive(PaintPrimitive::Image(ImagePrimitive {
                rect,
                content: ImageContent::Decoded(image.clone()),
                tint,
            })));
    }

    fn push_clip(&mut self, rect: crate::Rect) {
        self.ops.push(PaintOp::PushClip(rect));
    }

    fn push_clip_rounded(&mut self, rect: crate::Rect, radius: f32) {
        self.ops.push(PaintOp::PushRoundedClip(rect, radius));
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
                italic: false,
                selection: None,
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
                italic: false,
                selection: None,
                underline: false,
                strikethrough: false,
            }))]
        );
    }

    #[test]
    fn recording_painter_keeps_italic_selection_and_image_pixels() {
        let mut painter = RecordingPainter::new(creamui_theme::ColorScheme::default());
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        };
        let color = creamui_theme::Color::rgb(1, 2, 3);
        painter.fill_text_font(
            rect,
            "hi",
            color,
            14.0,
            crate::TextAlign::Start,
            None,
            false,
            true,
        );
        painter.fill_text_selected_font(
            rect,
            "hello",
            color,
            creamui_theme::Color::rgb(4, 5, 6),
            1..4,
            14.0,
            crate::TextAlign::Start,
            Some("Inter"),
        );
        let image = crate::RgbaImage::new(1, 1, vec![1, 2, 3, 4]).unwrap();
        painter.push_clip_rounded(rect, 5.0);
        painter.draw_image(rect, &image, Some(color));
        painter.pop_clip();

        let ops = painter.into_ops();
        let PaintOp::Primitive(PaintPrimitive::Text(italic)) = &ops[0] else {
            panic!("expected italic text");
        };
        assert!(italic.italic);
        let PaintOp::Primitive(PaintPrimitive::Text(selected)) = &ops[1] else {
            panic!("expected selected text");
        };
        assert_eq!(
            selected.selection,
            Some((1..4, creamui_theme::Color::rgb(4, 5, 6)))
        );
        assert_eq!(selected.family.as_deref(), Some("Inter"));
        assert_eq!(ops[2], PaintOp::PushRoundedClip(rect, 5.0));
        let PaintOp::Primitive(PaintPrimitive::Image(recorded)) = &ops[3] else {
            panic!("expected image");
        };
        assert!(
            matches!(&recorded.content, ImageContent::Decoded(pixels) if pixels.id() == image.id())
        );
        assert_eq!(recorded.tint, Some(color));
        assert_eq!(ops[4], PaintOp::PopClip);
    }
}
