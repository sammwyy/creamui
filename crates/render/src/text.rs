//! Text layout and glyph rasterization via `fontdue`, cached across frames
//! so an unchanged label costs one hash lookup per paint.

use creamui_core::TextAlign;
use creamui_fonts::FontWeight;
use fontdue::layout::{
    CoordinateSystem, GlyphRasterConfig, HorizontalAlign, Layout, LayoutSettings, TextStyle,
    VerticalAlign,
};
use fontdue::Font as Face;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

const EVICT_EVERY_FRAMES: u64 = 120;
const EVICT_UNUSED_FOR_FRAMES: u64 = 600;

/// One rasterized glyph coverage mask, shared by every layout using it.
pub struct GlyphBitmap {
    pub key: GlyphRasterConfig,
    pub width: u32,
    pub height: u32,
    pub coverage: Box<[u8]>,
}

pub struct PlacedGlyph {
    pub bitmap: Rc<GlyphBitmap>,
    pub x: i32,
    pub y: i32,
    pub byte_offset: usize,
}

/// Glyphs positioned relative to the top-left of the text box they were
/// laid out in, in physical pixels.
pub struct TextLayout {
    pub glyphs: Vec<PlacedGlyph>,
    pub ink: [i32; 4],
}

struct LayoutEntry {
    face: usize,
    text: Box<str>,
    size: u32,
    width: u32,
    height: u32,
    align: TextAlign,
    layout: Rc<TextLayout>,
    used: u64,
}

pub struct TextSystem {
    faces: Vec<(Option<String>, bool, Rc<Face>)>,
    layout: Layout,
    layouts: HashMap<u64, LayoutEntry>,
    glyphs: HashMap<GlyphRasterConfig, (Rc<GlyphBitmap>, u64)>,
    frame: u64,
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSystem {
    pub fn new() -> Self {
        Self {
            faces: Vec::new(),
            layout: Layout::new(CoordinateSystem::PositiveYDown),
            layouts: HashMap::new(),
            glyphs: HashMap::new(),
            frame: 0,
        }
    }

    fn face(&mut self, family: Option<&str>, bold: bool) -> Rc<Face> {
        if let Some((_, _, face)) = self
            .faces
            .iter()
            .find(|(f, b, _)| f.as_deref() == family && *b == bold)
        {
            return face.clone();
        }
        let weight = if bold {
            FontWeight::Bold
        } else {
            FontWeight::Regular
        };
        let preferred = family
            .map(str::to_owned)
            .unwrap_or_else(creamui_fonts::preferred_family);
        let face = creamui_fonts::resolve(&preferred, weight);
        self.faces
            .push((family.map(str::to_owned), bold, face.clone()));
        face
    }

    /// Lays out `text` at `size` pixels inside a `width` x `height` box,
    /// centered vertically and aligned horizontally by `align`.
    #[allow(clippy::too_many_arguments)]
    pub fn layout(
        &mut self,
        family: Option<&str>,
        bold: bool,
        text: &str,
        size: f32,
        width: f32,
        height: f32,
        align: TextAlign,
    ) -> Rc<TextLayout> {
        let face = self.face(family, bold);
        let face_id = Rc::as_ptr(&face) as usize;
        let (size_bits, width_bits, height_bits) =
            (size.to_bits(), width.to_bits(), height.to_bits());
        let mut hasher = DefaultHasher::new();
        (
            face_id,
            text,
            size_bits,
            width_bits,
            height_bits,
            align as u8,
        )
            .hash(&mut hasher);
        let hash = hasher.finish();
        let frame = self.frame;
        if let Some(entry) = self.layouts.get_mut(&hash) {
            if entry.face == face_id
                && entry.size == size_bits
                && entry.width == width_bits
                && entry.height == height_bits
                && entry.align == align
                && &*entry.text == text
            {
                entry.used = frame;
                return entry.layout.clone();
            }
        }

        #[cfg(feature = "perf-metrics")]
        creamui_core::metrics::record(|m| m.text_layouts += 1);
        self.layout.reset(&LayoutSettings {
            max_width: Some(width),
            max_height: Some(height),
            horizontal_align: match align {
                TextAlign::Start => HorizontalAlign::Left,
                TextAlign::Center => HorizontalAlign::Center,
                TextAlign::End => HorizontalAlign::Right,
            },
            vertical_align: VerticalAlign::Middle,
            ..LayoutSettings::default()
        });
        self.layout
            .append(&[face.as_ref()], &TextStyle::new(text, size, 0));

        let mut ink = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
        let mut glyphs = Vec::with_capacity(self.layout.glyphs().len());
        for g in self.layout.glyphs() {
            if g.width == 0 || g.height == 0 {
                continue;
            }
            let bitmap = match self.glyphs.get_mut(&g.key) {
                Some((bitmap, used)) => {
                    *used = frame;
                    bitmap.clone()
                }
                None => {
                    let (metrics, coverage) = face.rasterize_config(g.key);
                    let bitmap = Rc::new(GlyphBitmap {
                        key: g.key,
                        width: metrics.width as u32,
                        height: metrics.height as u32,
                        coverage: coverage.into_boxed_slice(),
                    });
                    self.glyphs.insert(g.key, (bitmap.clone(), frame));
                    bitmap
                }
            };
            let x = g.x.round() as i32;
            let y = g.y.round() as i32;
            ink = [
                ink[0].min(x),
                ink[1].min(y),
                ink[2].max(x + bitmap.width as i32),
                ink[3].max(y + bitmap.height as i32),
            ];
            glyphs.push(PlacedGlyph {
                bitmap,
                x,
                y,
                byte_offset: g.byte_offset,
            });
        }
        if glyphs.is_empty() {
            ink = [0; 4];
        }
        let layout = Rc::new(TextLayout { glyphs, ink });
        self.layouts.insert(
            hash,
            LayoutEntry {
                face: face_id,
                text: text.into(),
                size: size_bits,
                width: width_bits,
                height: height_bits,
                align,
                layout: layout.clone(),
                used: frame,
            },
        );
        layout
    }

    /// Advances the cache clock, periodically dropping layouts and glyphs
    /// that no frame has used for a while.
    pub fn end_frame(&mut self) {
        self.frame += 1;
        if !self.frame.is_multiple_of(EVICT_EVERY_FRAMES) {
            return;
        }
        let horizon = self.frame.saturating_sub(EVICT_UNUSED_FOR_FRAMES);
        self.layouts.retain(|_, entry| entry.used >= horizon);
        self.glyphs.retain(|_, (_, used)| *used >= horizon);
        log::trace!(
            "creamui-render: text cache holds {} layouts, {} glyphs",
            self.layouts.len(),
            self.glyphs.len()
        );
    }

    pub fn cached_layouts(&self) -> usize {
        self.layouts.len()
    }

    pub fn cached_glyphs(&self) -> usize {
        self.glyphs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_requests_share_one_layout() {
        let mut text = TextSystem::new();
        let a = text.layout(None, false, "Hello", 14.0, 200.0, 20.0, TextAlign::Start);
        let b = text.layout(None, false, "Hello", 14.0, 200.0, 20.0, TextAlign::Start);
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(a.glyphs.len(), 5);
        let c = text.layout(None, true, "Hello", 14.0, 200.0, 20.0, TextAlign::Start);
        assert!(!Rc::ptr_eq(&a, &c));
        assert!(Rc::ptr_eq(&a.glyphs[2].bitmap, &a.glyphs[3].bitmap));
    }

    #[test]
    fn alignment_moves_glyphs_inside_the_box() {
        let mut text = TextSystem::new();
        let start = text.layout(None, false, "Hi", 14.0, 200.0, 20.0, TextAlign::Start);
        let end = text.layout(None, false, "Hi", 14.0, 200.0, 20.0, TextAlign::End);
        assert!(end.ink[0] > start.ink[0] + 100);
        assert!(end.ink[2] <= 200);
    }

    #[test]
    fn unused_entries_are_evicted() {
        let mut text = TextSystem::new();
        text.layout(None, false, "gone", 14.0, 100.0, 20.0, TextAlign::Start);
        for _ in 0..EVICT_UNUSED_FOR_FRAMES + EVICT_EVERY_FRAMES {
            text.end_frame();
        }
        assert_eq!(text.cached_layouts(), 0);
        assert_eq!(text.cached_glyphs(), 0);
    }
}
