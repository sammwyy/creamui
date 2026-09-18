//! Backend-independent frame description: an ordered list of primitives in
//! physical pixels, each carrying the clip it was recorded under.
//!
//! Recording a frame never touches pixels. Comparing two consecutive lists
//! yields the exact regions whose pixels can differ, which the CPU
//! rasterizer and presenters use to redraw and upload only those regions.

use crate::text::TextLayout;
use creamui_core::{Rect, RgbaImage, Size};
use creamui_theme::Color;
use std::ops::Range;
use std::rc::Rc;

const MAX_DAMAGE_RECTS: usize = 8;
const MAX_DAMAGE_AREA_RATIO: f32 = 0.5;
const ITALIC_SHEAR: f32 = 0.22;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Bounds {
    pub const EMPTY: Bounds = Bounds {
        x0: 0.0,
        y0: 0.0,
        x1: 0.0,
        y1: 0.0,
    };

    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Bounds { x0, y0, x1, y1 }
    }

    pub fn from_rect(rect: Rect, scale: f32) -> Self {
        Bounds {
            x0: rect.x * scale,
            y0: rect.y * scale,
            x1: (rect.x + rect.width) * scale,
            y1: (rect.y + rect.height) * scale,
        }
    }

    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }

    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }

    pub fn intersect(&self, other: Bounds) -> Bounds {
        let b = Bounds {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        };
        if b.is_empty() {
            Bounds::EMPTY
        } else {
            b
        }
    }

    pub fn contains(&self, other: Bounds) -> bool {
        other.x0 >= self.x0 && other.y0 >= self.y0 && other.x1 <= self.x1 && other.y1 <= self.y1
    }

    pub fn inflate(&self, amount: f32) -> Bounds {
        Bounds {
            x0: self.x0 - amount,
            y0: self.y0 - amount,
            x1: self.x1 + amount,
            y1: self.y1 + amount,
        }
    }

    pub fn round(&self) -> Bounds {
        Bounds {
            x0: self.x0.round(),
            y0: self.y0.round(),
            x1: self.x1.round(),
            y1: self.y1.round(),
        }
    }

    pub fn round_out(&self) -> Bounds {
        Bounds {
            x0: self.x0.floor(),
            y0: self.y0.floor(),
            x1: self.x1.ceil(),
            y1: self.y1.ceil(),
        }
    }

    pub fn to_rect(self) -> Rect {
        Rect {
            x: self.x0,
            y: self.y0,
            width: self.width(),
            height: self.height(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundedClip {
    pub bounds: Bounds,
    pub radius: f32,
}

impl RoundedClip {
    /// The largest axis-aligned rect fully inside the rounded shape.
    pub fn inner(&self) -> Bounds {
        let inset = self.radius * (1.0 - std::f32::consts::FRAC_1_SQRT_2);
        Bounds {
            x0: self.bounds.x0 + inset,
            y0: self.bounds.y0 + inset,
            x1: self.bounds.x1 - inset,
            y1: self.bounds.y1 - inset,
        }
    }
}

/// A pixel-aligned rect clip, optionally further restricted by the
/// innermost rounded clip in effect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clip {
    pub bounds: Bounds,
    pub rounded: Option<RoundedClip>,
}

impl Clip {
    /// Whether `bounds` is drawn identically with or without this clip.
    pub fn contains(&self, bounds: Bounds) -> bool {
        self.bounds.contains(bounds) && self.rounded.is_none_or(|r| r.inner().contains(bounds))
    }
}

/// A rounded rectangle with an optional inner border. `bounds` is the
/// outer edge; the border is drawn inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadGradient {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub start_color: Color,
    pub end_color: Color,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub bounds: Bounds,
    pub background: Color,
    pub gradient: Option<QuadGradient>,
    pub radius: f32,
    pub border_width: f32,
    pub border_color: Color,
}

/// A straight segment with round caps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub width: f32,
    pub color: Color,
}

#[derive(Clone)]
pub struct TextRun {
    pub layout: Rc<TextLayout>,
    pub x: i32,
    pub y: i32,
    pub color: Color,
    pub selection: Option<(Range<usize>, Color)>,
    pub italic: bool,
}

impl TextRun {
    pub fn glyph_color(&self, byte_offset: usize) -> Color {
        match &self.selection {
            Some((range, color)) if range.contains(&byte_offset) => *color,
            _ => self.color,
        }
    }

    /// Horizontal shift applied to glyph row `row` (counted from the top) of
    /// a glyph `height` rows tall, synthesizing an oblique face.
    pub fn shear(&self, row: u32, height: u32) -> i32 {
        if self.italic {
            ((height.saturating_sub(1 + row)) as f32 * ITALIC_SHEAR).round() as i32
        } else {
            0
        }
    }

    pub fn shear_factor(&self) -> f32 {
        if self.italic {
            ITALIC_SHEAR
        } else {
            0.0
        }
    }
}

impl PartialEq for TextRun {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.layout, &other.layout)
            && self.x == other.x
            && self.y == other.y
            && self.color == other.color
            && self.selection == other.selection
            && self.italic == other.italic
    }
}

#[derive(Clone)]
pub struct ImagePrimitive {
    pub bounds: Bounds,
    pub image: RgbaImage,
    pub tint: Option<Color>,
}

impl PartialEq for ImagePrimitive {
    fn eq(&self, other: &Self) -> bool {
        self.bounds == other.bounds
            && self.image.id() == other.image.id()
            && self.tint == other.tint
    }
}

#[derive(Clone, PartialEq)]
pub enum Primitive {
    Quad(Quad),
    Line(Line),
    Text(TextRun),
    Image(ImagePrimitive),
}

impl Primitive {
    /// Every pixel the primitive may touch, including antialiasing.
    pub fn bounds(&self) -> Bounds {
        match self {
            Primitive::Quad(quad) => quad.bounds.inflate(1.0),
            Primitive::Line(line) => {
                let pad = line.width / 2.0 + 1.0;
                Bounds {
                    x0: line.from[0].min(line.to[0]) - pad,
                    y0: line.from[1].min(line.to[1]) - pad,
                    x1: line.from[0].max(line.to[0]) + pad,
                    y1: line.from[1].max(line.to[1]) + pad,
                }
            }
            Primitive::Text(run) => {
                let [x0, y0, x1, y1] = run.layout.ink;
                let shear = run.shear_factor() * (y1 - y0) as f32;
                Bounds {
                    x0: (run.x + x0) as f32,
                    y0: (run.y + y0) as f32,
                    x1: (run.x + x1) as f32 + shear.ceil(),
                    y1: (run.y + y1) as f32,
                }
            }
            Primitive::Image(image) => image.bounds.inflate(1.0),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct DrawItem {
    pub primitive: Primitive,
    pub clip: Clip,
}

impl DrawItem {
    pub fn visible_bounds(&self) -> Bounds {
        self.primitive.bounds().intersect(self.clip.bounds)
    }
}

pub struct DisplayList {
    pub width: u32,
    pub height: u32,
    pub clear: Color,
    pub items: Vec<DrawItem>,
}

impl DisplayList {
    pub fn new(width: u32, height: u32, clear: Color) -> Self {
        DisplayList {
            width,
            height,
            clear,
            items: Vec::new(),
        }
    }

    pub fn viewport(&self) -> Bounds {
        Bounds::new(0.0, 0.0, self.width as f32, self.height as f32)
    }
}

/// Pixel regions that differ between two frames, in physical pixels.
#[derive(Clone, Debug, PartialEq)]
pub enum Damage {
    None,
    Full,
    Partial(Vec<Bounds>),
}

impl Damage {
    pub fn is_none(&self) -> bool {
        matches!(self, Damage::None)
    }

    pub fn regions(&self, viewport: Bounds) -> Vec<Bounds> {
        match self {
            Damage::None => Vec::new(),
            Damage::Full => vec![viewport],
            Damage::Partial(regions) => regions.clone(),
        }
    }

    pub fn area(&self, viewport: Bounds) -> f32 {
        self.regions(viewport)
            .iter()
            .fold(0.0, |area, b| area + b.width() * b.height())
    }
}

/// Diffs `next` against `previous`. Items outside the common prefix and
/// suffix are compared pairwise when both frames changed the same number of
/// items, otherwise every item in the differing span is damaged — which
/// also covers a paint-order change.
pub fn damage(previous: Option<&DisplayList>, next: &DisplayList) -> Damage {
    let Some(previous) = previous else {
        return Damage::Full;
    };
    if previous.width != next.width
        || previous.height != next.height
        || previous.clear != next.clear
    {
        return Damage::Full;
    }
    let (old, new) = (&previous.items, &next.items);
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let max_suffix = old.len().min(new.len()) - prefix;
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count();
    let old_mid = &old[prefix..old.len() - suffix];
    let new_mid = &new[prefix..new.len() - suffix];

    let mut rects = Vec::new();
    let mut push = |item: &DrawItem| {
        let bounds = item.visible_bounds().round_out();
        if !bounds.is_empty() {
            rects.push(bounds.to_rect());
        }
    };
    if old_mid.len() == new_mid.len() {
        for (a, b) in old_mid.iter().zip(new_mid) {
            if a != b {
                push(a);
                push(b);
            }
        }
    } else {
        old_mid.iter().chain(new_mid).for_each(&mut push);
    }
    if rects.is_empty() {
        return Damage::None;
    }
    let viewport = Size {
        width: next.width as f32,
        height: next.height as f32,
    };
    let merged =
        creamui_core::merge_damage(&rects, viewport, MAX_DAMAGE_AREA_RATIO, MAX_DAMAGE_RECTS);
    let view = next.viewport();
    let regions: Vec<Bounds> = merged
        .iter()
        .map(|r| Bounds::from_rect(*r, 1.0).intersect(view))
        .filter(|b| !b.is_empty())
        .collect();
    match regions.as_slice() {
        [] => Damage::None,
        [only] if *only == view => Damage::Full,
        _ => Damage::Partial(regions),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(x: f32, color: Color) -> DrawItem {
        DrawItem {
            primitive: Primitive::Quad(Quad {
                bounds: Bounds::new(x, 10.0, x + 10.0, 20.0),
                background: color,
                gradient: None,
                radius: 0.0,
                border_width: 0.0,
                border_color: Color::rgba(0, 0, 0, 0),
            }),
            clip: Clip {
                bounds: Bounds::new(0.0, 0.0, 200.0, 100.0),
                rounded: None,
            },
        }
    }

    fn list(items: Vec<DrawItem>) -> DisplayList {
        DisplayList {
            width: 200,
            height: 100,
            clear: Color::rgb(0, 0, 0),
            items,
        }
    }

    const RED: Color = Color::rgb(255, 0, 0);
    const BLUE: Color = Color::rgb(0, 0, 255);

    #[test]
    fn first_frame_and_resizes_are_full_damage() {
        let a = list(vec![quad(0.0, RED)]);
        assert_eq!(damage(None, &a), Damage::Full);
        let mut b = list(vec![quad(0.0, RED)]);
        b.width = 300;
        assert_eq!(damage(Some(&a), &b), Damage::Full);
    }

    #[test]
    fn identical_frames_have_no_damage() {
        let a = list(vec![quad(0.0, RED), quad(50.0, BLUE)]);
        let b = list(vec![quad(0.0, RED), quad(50.0, BLUE)]);
        assert_eq!(damage(Some(&a), &b), Damage::None);
    }

    #[test]
    fn a_changed_item_damages_only_its_bounds() {
        let a = list(vec![quad(0.0, RED), quad(50.0, BLUE), quad(100.0, RED)]);
        let b = list(vec![quad(0.0, RED), quad(50.0, RED), quad(100.0, RED)]);
        assert_eq!(
            damage(Some(&a), &b),
            Damage::Partial(vec![Bounds::new(49.0, 9.0, 61.0, 21.0)])
        );
    }

    #[test]
    fn scattered_changes_stay_separate() {
        let a = list(vec![quad(0.0, RED), quad(50.0, BLUE), quad(150.0, RED)]);
        let b = list(vec![quad(0.0, BLUE), quad(50.0, BLUE), quad(150.0, BLUE)]);
        let Damage::Partial(regions) = damage(Some(&a), &b) else {
            panic!("expected partial damage");
        };
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn a_moved_item_damages_old_and_new_positions() {
        let a = list(vec![quad(0.0, RED)]);
        let b = list(vec![quad(100.0, RED)]);
        let Damage::Partial(regions) = damage(Some(&a), &b) else {
            panic!("expected partial damage");
        };
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn an_insertion_damages_the_inserted_item() {
        let a = list(vec![quad(0.0, RED), quad(100.0, RED)]);
        let b = list(vec![quad(0.0, RED), quad(50.0, BLUE), quad(100.0, RED)]);
        assert_eq!(
            damage(Some(&a), &b),
            Damage::Partial(vec![Bounds::new(49.0, 9.0, 61.0, 21.0)])
        );
    }

    #[test]
    fn clipped_items_only_damage_their_visible_part() {
        let mut hidden = quad(0.0, RED);
        hidden.clip.bounds = Bounds::new(150.0, 0.0, 200.0, 100.0);
        let a = list(vec![hidden.clone()]);
        let mut b = list(vec![hidden]);
        if let Primitive::Quad(q) = &mut b.items[0].primitive {
            q.background = BLUE;
        }
        assert_eq!(damage(Some(&a), &b), Damage::None);
    }

    #[test]
    fn large_damage_collapses_to_full() {
        let big = |color| DrawItem {
            primitive: Primitive::Quad(Quad {
                bounds: Bounds::new(0.0, 0.0, 180.0, 90.0),
                background: color,
                gradient: None,
                radius: 0.0,
                border_width: 0.0,
                border_color: Color::rgba(0, 0, 0, 0),
            }),
            clip: Clip {
                bounds: Bounds::new(0.0, 0.0, 200.0, 100.0),
                rounded: None,
            },
        };
        let a = list(vec![big(RED)]);
        let b = list(vec![big(BLUE)]);
        assert_eq!(damage(Some(&a), &b), Damage::Full);
    }
}
