//! Backend-independent frame description: an ordered list of primitives in
//! physical pixels, each carrying the clip it was recorded under.
//!
//! Recording a frame never touches pixels. Comparing two consecutive lists
//! yields the exact regions whose pixels can differ, which the CPU
//! rasterizer and presenters use to redraw and upload only those regions.

use crate::text::TextLayout;
use creamui_core::{Rect, RgbaImage, Size};
use creamui_theme::Color;
use std::borrow::Cow;
use std::cell::RefCell;
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

    pub fn translate(&self, [dx, dy]: [f32; 2]) -> Bounds {
        Bounds {
            x0: self.x0 + dx,
            y0: self.y0 + dy,
            x1: self.x1 + dx,
            y1: self.y1 + dy,
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
    /// The primitive moved by `offset`; text lands on the nearest pixel.
    pub fn translate(&self, offset: [f32; 2]) -> Primitive {
        let [dx, dy] = offset;
        match self {
            Primitive::Quad(quad) => Primitive::Quad(Quad {
                bounds: quad.bounds.translate(offset),
                gradient: quad.gradient.map(|gradient| QuadGradient {
                    start: [gradient.start[0] + dx, gradient.start[1] + dy],
                    end: [gradient.end[0] + dx, gradient.end[1] + dy],
                    ..gradient
                }),
                ..*quad
            }),
            Primitive::Line(line) => Primitive::Line(Line {
                from: [line.from[0] + dx, line.from[1] + dy],
                to: [line.to[0] + dx, line.to[1] + dy],
                ..*line
            }),
            Primitive::Text(run) => Primitive::Text(TextRun {
                x: run.x + dx.round() as i32,
                y: run.y + dy.round() as i32,
                ..run.clone()
            }),
            Primitive::Image(image) => Primitive::Image(ImagePrimitive {
                bounds: image.bounds.translate(offset),
                ..image.clone()
            }),
        }
    }

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
    /// In the item's layer's content space.
    pub clip: Clip,
    /// The scroll layer the item was recorded in; `0` is the frame itself.
    pub layer: u16,
}

impl DrawItem {
    /// The clip in screen space, including the item's layer viewports.
    pub fn screen_clip(&self, spaces: &[LayerSpace]) -> Clip {
        if self.layer == 0 {
            return self.clip;
        }
        let space = &spaces[self.layer as usize];
        Clip {
            bounds: self
                .clip
                .bounds
                .translate(space.offset)
                .intersect(space.clip.bounds),
            rounded: self
                .clip
                .rounded
                .map(|rounded| RoundedClip {
                    bounds: rounded.bounds.translate(space.offset),
                    radius: rounded.radius,
                })
                .or(space.clip.rounded),
        }
    }

    /// The primitive in screen space.
    pub fn screen_primitive(&self, spaces: &[LayerSpace]) -> Cow<'_, Primitive> {
        let offset = spaces[self.layer as usize].offset;
        if offset == [0.0; 2] {
            Cow::Borrowed(&self.primitive)
        } else {
            Cow::Owned(self.primitive.translate(offset))
        }
    }

    /// Screen pixels the item can touch.
    pub fn visible_bounds(&self, spaces: &[LayerSpace]) -> Bounds {
        let offset = spaces[self.layer as usize].offset;
        self.primitive
            .bounds()
            .translate(offset)
            .intersect(self.screen_clip(spaces).bounds)
    }
}

/// A scrolled region. Its items are recorded in the layer's own content
/// space, so scrolling changes only `offset`, never the items.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollLayer {
    /// The enclosing layer; `0` is the frame itself.
    pub parent: u16,
    /// Visible region, in the parent's content space.
    pub viewport: Bounds,
    pub radius: f32,
    /// Added to content coordinates to reach the parent's content space.
    pub offset: [f32; 2],
}

/// Where a layer's content lands on screen in one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerSpace {
    pub offset: [f32; 2],
    pub clip: Clip,
}

pub struct DisplayList {
    pub width: u32,
    pub height: u32,
    pub clear: Color,
    pub items: Vec<DrawItem>,
    /// Layer `n` is `layers[n - 1]`.
    pub layers: Vec<ScrollLayer>,
}

impl DisplayList {
    pub fn new(width: u32, height: u32, clear: Color) -> Self {
        DisplayList {
            width,
            height,
            clear,
            items: Vec::new(),
            layers: Vec::new(),
        }
    }

    pub fn viewport(&self) -> Bounds {
        Bounds::new(0.0, 0.0, self.width as f32, self.height as f32)
    }

    /// Screen placement of every layer, indexed by layer id.
    pub fn spaces(&self) -> Vec<LayerSpace> {
        let mut spaces = Vec::with_capacity(self.layers.len() + 1);
        spaces.push(LayerSpace {
            offset: [0.0; 2],
            clip: Clip {
                bounds: self.viewport(),
                rounded: None,
            },
        });
        for layer in &self.layers {
            let parent = spaces[layer.parent as usize];
            let viewport = layer.viewport.translate(parent.offset);
            spaces.push(LayerSpace {
                offset: [
                    parent.offset[0] + layer.offset[0],
                    parent.offset[1] + layer.offset[1],
                ],
                clip: Clip {
                    bounds: viewport.intersect(parent.clip.bounds),
                    rounded: if layer.radius > 0.0 {
                        Some(RoundedClip {
                            bounds: viewport,
                            radius: layer.radius,
                        })
                    } else {
                        parent.clip.rounded
                    },
                },
            });
        }
        spaces
    }

    fn within(&self, layer: u16, ancestor: u16) -> bool {
        let mut current = layer;
        loop {
            if current == ancestor {
                return true;
            }
            if current == 0 {
                return false;
            }
            current = self.layers[current as usize - 1].parent;
        }
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

    fn merged(rects: &[Rect], viewport: Bounds) -> Damage {
        if rects.is_empty() {
            return Damage::None;
        }
        let size = Size {
            width: viewport.width(),
            height: viewport.height(),
        };
        let merged =
            creamui_core::merge_damage(rects, size, MAX_DAMAGE_AREA_RATIO, MAX_DAMAGE_RECTS);
        let regions: Vec<Bounds> = merged
            .iter()
            .map(|r| Bounds::from_rect(*r, 1.0).intersect(viewport))
            .filter(|b| !b.is_empty())
            .collect();
        match regions.as_slice() {
            [] => Damage::None,
            [only] if *only == viewport => Damage::Full,
            _ => Damage::Partial(regions),
        }
    }
}

/// Pixels of the previous frame inside `area` that reappear shifted by
/// `(dx, dy)`, as when one scroll layer moved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollBlit {
    pub area: Bounds,
    pub dx: i32,
    pub dy: i32,
}

/// How to turn the previous frame's pixels into the next frame's: shift
/// `scroll` if set, then redraw `repaint`.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameDiff {
    pub scroll: Option<ScrollBlit>,
    pub repaint: Damage,
}

impl FrameDiff {
    pub fn is_none(&self) -> bool {
        self.scroll.is_none() && self.repaint.is_none()
    }

    /// Every region whose pixels change on screen.
    pub fn changed(&self, viewport: Bounds) -> Damage {
        let Some(scroll) = self.scroll else {
            return self.repaint.clone();
        };
        match &self.repaint {
            Damage::Full => Damage::Full,
            Damage::None => Damage::Partial(vec![scroll.area]),
            Damage::Partial(regions) => {
                let rects: Vec<Rect> = regions
                    .iter()
                    .chain([&scroll.area])
                    .map(|b| b.to_rect())
                    .collect();
                Damage::merged(&rects, viewport)
            }
        }
    }
}

/// Diffs `next` against `previous`; see [`diff`]. Returns every region
/// whose pixels change.
pub fn damage(previous: Option<&DisplayList>, next: &DisplayList) -> Damage {
    diff(previous, next).changed(next.viewport())
}

/// The one layer whose offset changed by whole pixels, with everything else
/// about the layers unchanged; its already drawn content can be shifted.
fn scrolled_layer(previous: &DisplayList, next: &DisplayList) -> Option<(u16, i32, i32)> {
    if previous.layers.len() != next.layers.len() {
        return None;
    }
    let mut moved = None;
    for (index, (old, new)) in previous.layers.iter().zip(&next.layers).enumerate() {
        if old == new {
            continue;
        }
        if moved.is_some()
            || old.parent != new.parent
            || old.viewport != new.viewport
            || old.radius != new.radius
        {
            return None;
        }
        let (dx, dy) = (new.offset[0] - old.offset[0], new.offset[1] - old.offset[1]);
        if dx.fract() != 0.0 || dy.fract() != 0.0 {
            return None;
        }
        moved = Some((index as u16 + 1, dx as i32, dy as i32));
    }
    moved
}

/// Whether everything drawn under `area` before the items at `first` is a
/// single color, so shifting those pixels leaves them unchanged.
fn uniform_backdrop(list: &DisplayList, spaces: &[LayerSpace], first: usize, area: Bounds) -> bool {
    let backdrop = list.items[..first]
        .iter()
        .rev()
        .find(|item| !item.visible_bounds(spaces).intersect(area).is_empty());
    let Some(item) = backdrop else {
        return true;
    };
    let Primitive::Quad(quad) = item.screen_primitive(spaces).into_owned() else {
        return false;
    };
    let clip = item.screen_clip(spaces);
    quad.gradient.is_none()
        && quad.background.a == 255
        && quad.border_width == 0.0
        && clip.contains(area)
        && RoundedClip {
            bounds: quad.bounds,
            radius: quad.radius,
        }
        .inner()
        .contains(area)
}

/// Rects of `area` not covered by `area` shifted by `(dx, dy)`.
fn exposed(area: Bounds, dx: i32, dy: i32) -> Vec<Bounds> {
    let (dx, dy) = (dx as f32, dy as f32);
    let mut strips = Vec::new();
    if dy > 0.0 {
        strips.push(Bounds::new(area.x0, area.y0, area.x1, area.y0 + dy));
    } else if dy < 0.0 {
        strips.push(Bounds::new(area.x0, area.y1 + dy, area.x1, area.y1));
    }
    if dx > 0.0 {
        strips.push(Bounds::new(area.x0, area.y0, area.x0 + dx, area.y1));
    } else if dx < 0.0 {
        strips.push(Bounds::new(area.x1 + dx, area.y0, area.x1, area.y1));
    }
    strips
        .into_iter()
        .map(|strip| strip.intersect(area))
        .filter(|strip| !strip.is_empty())
        .collect()
}

/// How far ahead [`pair`] looks for an item that one frame added or
/// dropped, e.g. rows and their text entering or leaving a scrolled
/// viewport.
const ALIGN_LOOKAHEAD: usize = 8;

/// Walks `old` and `new` in step, calling `removed`/`added` for items only
/// one side has (found within [`ALIGN_LOOKAHEAD`] items) and `changed` for
/// positions whose items differ.
fn pair<T>(
    old: &[T],
    new: &[T],
    equal: impl Fn(&T, &T) -> bool,
    mut removed: impl FnMut(&T),
    mut added: impl FnMut(&T),
    mut changed: impl FnMut(&T, &T),
) {
    let (mut i, mut j) = (0, 0);
    while i < old.len() && j < new.len() {
        if equal(&old[i], &new[j]) {
            i += 1;
            j += 1;
            continue;
        }
        let ahead = |items: &[T], from: usize, target: &T, flip: bool| {
            items[from..].iter().take(ALIGN_LOOKAHEAD).position(|item| {
                if flip {
                    equal(target, item)
                } else {
                    equal(item, target)
                }
            })
        };
        if let Some(skip) = ahead(old, i + 1, &new[j], false) {
            old[i..=i + skip].iter().for_each(&mut removed);
            i += skip + 1;
        } else if let Some(skip) = ahead(new, j + 1, &old[i], true) {
            new[j..=j + skip].iter().for_each(&mut added);
            j += skip + 1;
        } else {
            changed(&old[i], &new[j]);
            i += 1;
            j += 1;
        }
    }
    old[i..].iter().for_each(removed);
    new[j..].iter().for_each(added);
}

/// Diffs `next` against `previous`. Items outside the common prefix and
/// suffix are paired in order, skipping items only one frame has; those
/// and changed pairs are damaged.
/// When only one scroll layer moved, by whole pixels, over a solid
/// backdrop, its pixels are shifted instead of redrawn and only what that
/// cannot reproduce is repainted.
pub fn diff(previous: Option<&DisplayList>, next: &DisplayList) -> FrameDiff {
    let full = FrameDiff {
        scroll: None,
        repaint: Damage::Full,
    };
    let Some(previous) = previous else {
        return full;
    };
    if previous.width != next.width
        || previous.height != next.height
        || previous.clear != next.clear
    {
        return full;
    }
    let old_spaces = previous.spaces();
    let new_spaces = next.spaces();

    let scroll = scrolled_layer(previous, next).and_then(|(layer, dx, dy)| {
        let area = new_spaces[layer as usize].clip.bounds.round();
        let first = next
            .items
            .iter()
            .position(|item| next.within(item.layer, layer))?;
        let fits =
            dx.unsigned_abs() < area.width() as u32 && dy.unsigned_abs() < area.height() as u32;
        (fits && !area.is_empty() && uniform_backdrop(next, &new_spaces, first, area))
            .then_some((layer, ScrollBlit { area, dx, dy }))
    });
    let shifted = |layer: u16| scroll.is_some_and(|(moved, _)| next.within(layer, moved));
    let same_space = |layer: u16| old_spaces.get(layer as usize) == new_spaces.get(layer as usize);
    let equal = |a: &DrawItem, b: &DrawItem| a == b && (shifted(a.layer) || same_space(a.layer));

    let (old, new) = (&previous.items, &next.items);
    let prefix = old.iter().zip(new).take_while(|(a, b)| equal(a, b)).count();
    let max_suffix = old.len().min(new.len()) - prefix;
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(max_suffix)
        .take_while(|(a, b)| equal(a, b))
        .count();
    let old_mid = &old[prefix..old.len() - suffix];
    let new_mid = &new[prefix..new.len() - suffix];

    let rects = RefCell::new(Vec::new());
    let push = |bounds: Bounds| {
        let bounds = bounds.round_out();
        if !bounds.is_empty() {
            rects.borrow_mut().push(bounds.to_rect());
        }
    };
    let old_bounds = |item: &DrawItem| {
        let spaces = if shifted(item.layer) {
            &new_spaces
        } else {
            &old_spaces
        };
        item.visible_bounds(spaces)
    };
    pair(
        old_mid,
        new_mid,
        equal,
        |item| push(old_bounds(item)),
        |item| push(item.visible_bounds(&new_spaces)),
        |a, b| {
            push(old_bounds(a));
            push(b.visible_bounds(&new_spaces));
        },
    );

    if let Some((layer, blit)) = scroll {
        for strip in exposed(blit.area, blit.dx, blit.dy) {
            push(strip);
        }
        let radius = new_spaces[layer as usize]
            .clip
            .rounded
            .map_or(0.0, |rounded| rounded.radius)
            .ceil();
        if radius > 0.0 {
            let a = blit.area;
            for (x, y) in [
                (a.x0, a.y0),
                (a.x1 - radius, a.y0),
                (a.x0, a.y1 - radius),
                (a.x1 - radius, a.y1 - radius),
            ] {
                push(Bounds::new(x, y, x + radius, y + radius));
            }
        }
        let delta = [blit.dx as f32, blit.dy as f32];
        let above = old
            .iter()
            .rposition(|item| previous.within(item.layer, layer))
            .map_or(old.len(), |last| last + 1);
        for item in &old[above..] {
            let bounds = item.visible_bounds(&old_spaces);
            if !bounds.intersect(blit.area).is_empty() {
                push(bounds);
                push(bounds.translate(delta).intersect(blit.area));
            }
        }
    }

    FrameDiff {
        scroll: scroll.map(|(_, blit)| blit),
        repaint: Damage::merged(&rects.into_inner(), next.viewport()),
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
            layer: 0,
        }
    }

    fn list(items: Vec<DrawItem>) -> DisplayList {
        DisplayList {
            width: 200,
            height: 100,
            clear: Color::rgb(0, 0, 0),
            items,
            layers: Vec::new(),
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
            layer: 0,
        };
        let a = list(vec![big(RED)]);
        let b = list(vec![big(BLUE)]);
        assert_eq!(damage(Some(&a), &b), Damage::Full);
    }

    #[test]
    fn pairing_skips_items_only_one_side_has() {
        let (mut removed, mut added, mut changed) = (Vec::new(), Vec::new(), Vec::new());
        pair(
            &[1, 2, 3, 4, 5, 6],
            &[2, 3, 30, 5, 6, 7, 8],
            |a, b| a == b,
            |x| removed.push(*x),
            |x| added.push(*x),
            |a, b| changed.push((*a, *b)),
        );
        assert_eq!(removed, vec![1]);
        assert_eq!(changed, vec![(4, 30)]);
        assert_eq!(added, vec![7, 8]);
    }

    #[test]
    fn a_scrolled_layer_over_a_solid_backdrop_blits() {
        let backdrop = DrawItem {
            primitive: Primitive::Quad(Quad {
                bounds: Bounds::new(0.0, 0.0, 200.0, 100.0),
                background: RED,
                gradient: None,
                radius: 0.0,
                border_width: 0.0,
                border_color: Color::rgba(0, 0, 0, 0),
            }),
            clip: Clip {
                bounds: Bounds::new(0.0, 0.0, 200.0, 100.0),
                rounded: None,
            },
            layer: 0,
        };
        let scrolled = |offset: f32, rows: std::ops::Range<usize>| {
            let mut frame = list(vec![backdrop.clone()]);
            frame.layers.push(ScrollLayer {
                parent: 0,
                viewport: Bounds::new(0.0, 0.0, 200.0, 100.0),
                radius: 0.0,
                offset: [0.0, -offset],
            });
            for row in rows {
                let mut item = quad(0.0, BLUE);
                if let Primitive::Quad(q) = &mut item.primitive {
                    q.bounds = Bounds::new(0.0, row as f32 * 20.0, 200.0, row as f32 * 20.0 + 18.0);
                }
                item.layer = 1;
                frame.items.push(item);
            }
            frame
        };
        let diff = diff(Some(&scrolled(0.0, 0..5)), &scrolled(20.0, 1..6));
        assert_eq!(
            diff.scroll,
            Some(ScrollBlit {
                area: Bounds::new(0.0, 0.0, 200.0, 100.0),
                dx: 0,
                dy: -20,
            })
        );
        let Damage::Partial(regions) = &diff.repaint else {
            panic!("expected partial damage");
        };
        assert!(regions.iter().all(|region| region.y0 >= 79.0));
        assert_eq!(
            diff.repaint.area(Bounds::new(0.0, 0.0, 200.0, 100.0)),
            200.0 * 21.0
        );
    }
}
