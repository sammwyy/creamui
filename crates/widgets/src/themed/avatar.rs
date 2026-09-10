use super::*;
use creamui_core::layout::{LengthPercentageAuto, Position, Rect as InsetRect};

/// A circular avatar frame: image content or initials, an optional ring
/// border, and an optional status dot anchored at the bottom-right corner.
///
/// `.image(child)` expects `child` to already be sized to fill the avatar's
/// own box (e.g. `Image::new(data).layout(fixed(size, size)).fit(ImageFit::Cover)`) —
/// `Avatar` only clips it into a circle, it does not resize it.
pub struct Avatar {
    size: f32,
    theme: Theme,
    background: Color,
    content: Option<BoxedWidget>,
    initials: Option<String>,
    initials_color: Option<Color>,
    ring: Option<(Color, f32)>,
    status: Option<Color>,
}

impl Avatar {
    pub fn new(size: f32) -> Self {
        let theme = use_theme();
        Self {
            size,
            theme,
            background: theme.surface_hover,
            content: None,
            initials: None,
            initials_color: None,
            ring: None,
            status: None,
        }
    }

    /// Fallback fill color shown behind initials, or while no image is set.
    pub fn fallback_color(mut self, color: Color) -> Self {
        self.background = color;
        self
    }

    /// Content clipped into the circle — typically an `Image`. Takes
    /// priority over `.initials()` when both are set.
    pub fn image(mut self, content: BoxedWidget) -> Self {
        self.content = Some(content);
        self
    }

    /// Short fallback text (e.g. "JD") centered in the circle when no image
    /// is set.
    pub fn initials(mut self, text: impl Into<String>) -> Self {
        self.initials = Some(text.into());
        self
    }

    pub fn initials_color(mut self, color: Color) -> Self {
        self.initials_color = Some(color);
        self
    }

    /// A stroked ring around the circle, e.g. to mark the active conversation.
    pub fn ring(mut self, color: Color, width: f32) -> Self {
        self.ring = Some((color, width));
        self
    }

    /// A small colored dot at the bottom-right corner (online/away/offline).
    pub fn status(mut self, color: Color) -> Self {
        self.status = Some(color);
        self
    }
}

/// The clipped circular face: background, image content, initials, and ring.
/// Kept separate from the status dot so the dot is never itself clipped by
/// the circle it sits on the edge of.
struct AvatarFace {
    size: f32,
    background: Color,
    content: Option<BoxedWidget>,
    initials: Option<String>,
    initials_color: Color,
    ring: Option<(Color, f32)>,
}

impl Widget for AvatarFace {
    fn style(&self) -> creamui_core::Style {
        Style {
            size: crate::layout::fixed(self.size, self.size),
            flex_shrink: 0.,
            ..Default::default()
        }
        .into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.fill_rect(rect, self.background, self.size / 2.);
        if self.content.is_none() {
            if let Some(initials) = &self.initials {
                painter.fill_text_weight(
                    rect,
                    initials,
                    self.initials_color,
                    self.size * 0.4,
                    TextAlign::Center,
                    true,
                    false,
                );
            }
        }
        if let Some((color, width)) = self.ring {
            painter.stroke_rect(rect, color, width, self.size / 2.);
        }
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.content.take().into_iter().collect()
    }
    fn clips_children(&self) -> bool {
        true
    }
    fn clip_corner_radius(&self) -> f32 {
        self.size / 2.
    }
}

/// A small dot with a ring cut-out (page-background color) so it reads
/// clearly against whatever the avatar's image or fill color is.
struct AvatarStatusDot {
    size: f32,
    color: Color,
    ring_color: Color,
}

impl Widget for AvatarStatusDot {
    fn style(&self) -> creamui_core::Style {
        Style {
            position: Position::Absolute,
            inset: InsetRect {
                left: LengthPercentageAuto::Auto,
                top: LengthPercentageAuto::Auto,
                right: LengthPercentageAuto::Length(-self.size * 0.1),
                bottom: LengthPercentageAuto::Length(-self.size * 0.1),
            },
            size: crate::layout::fixed(self.size, self.size),
            flex_shrink: 0.,
            ..Default::default()
        }
        .into()
    }
    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        painter.fill_rect(rect, self.ring_color, self.size / 2.);
        let border = (self.size * 0.16).max(1.5);
        painter.fill_rect(
            Rect {
                x: rect.x + border,
                y: rect.y + border,
                width: rect.width - border * 2.,
                height: rect.height - border * 2.,
            },
            self.color,
            self.size / 2.,
        );
    }
}

impl Widget for Avatar {
    fn style(&self) -> creamui_core::Style {
        Style {
            size: crate::layout::fixed(self.size, self.size),
            flex_shrink: 0.,
            ..Default::default()
        }
        .into()
    }
    fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
    fn children(&mut self) -> Vec<BoxedWidget> {
        let mut children: Vec<BoxedWidget> = vec![Box::new(AvatarFace {
            size: self.size,
            background: self.background,
            content: self.content.take(),
            initials: self.initials.take(),
            initials_color: self.initials_color.unwrap_or(self.theme.text_primary),
            ring: self.ring,
        })];
        if let Some(color) = self.status {
            children.push(Box::new(AvatarStatusDot {
                size: (self.size * 0.3).max(9.),
                color,
                ring_color: self.theme.surface,
            }));
        }
        children
    }
}
