//! Text layout and glyph rasterization, cached across frames and shared by
//! every window on the UI thread, so an unchanged label costs one hash
//! lookup per paint and each glyph is rasterized once per process.

use creamui_core::TextAlign;
use creamui_fonts::{FontFace, FontWeight, HorizontalAlign, LayoutSettings};
use rustc_hash::{FxHashMap, FxHasher};
use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::Format;

const EVICT_EVERY_FRAMES: u64 = 120;
const EVICT_UNUSED_FOR_FRAMES: u64 = 600;

thread_local! {
    static SHARED: Rc<RefCell<TextSystem>> = Rc::new(RefCell::new(TextSystem::new()));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub face: u64,
    pub glyph: u16,
    pub px: u32,
}

/// One rasterized glyph coverage mask, shared by every layout using it.
/// `left`/`top` place it relative to the glyph's pen position on the
/// baseline.
pub struct GlyphBitmap {
    pub key: GlyphKey,
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
    pub coverage: Box<[u8]>,
    /// The GPU atlas (by id) and slot this glyph was last found in.
    pub atlas_slot: Cell<Option<(u64, u32)>>,
}

pub struct PlacedGlyph {
    pub bitmap: Rc<GlyphBitmap>,
    pub x: i32,
    pub y: i32,
    pub byte_offset: usize,
}

/// Glyphs positioned relative to the top-left of the text block, in
/// physical pixels. `height` is the block's line height total, which a
/// caller centers within its own box.
pub struct TextLayout {
    pub glyphs: Vec<PlacedGlyph>,
    pub ink: [i32; 4],
    pub height: f32,
}

struct LayoutEntry {
    face: u64,
    text: Box<str>,
    size: u32,
    width: u32,
    align: TextAlign,
    layout: Rc<TextLayout>,
    used: u64,
}

pub struct TextSystem {
    faces: Vec<(Option<String>, bool, Rc<FontFace>)>,
    scaler: ScaleContext,
    layouts: FxHashMap<u64, LayoutEntry>,
    glyphs: FxHashMap<GlyphKey, (Rc<GlyphBitmap>, u64)>,
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
            scaler: ScaleContext::new(),
            layouts: FxHashMap::default(),
            glyphs: FxHashMap::default(),
            frame: 0,
        }
    }

    /// The text system every window on this thread records with.
    pub fn shared() -> Rc<RefCell<TextSystem>> {
        SHARED.with(Rc::clone)
    }

    fn face(&mut self, family: Option<&str>, bold: bool) -> Rc<FontFace> {
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

    fn glyph(&mut self, face: &FontFace, key: GlyphKey, size: f32) -> Rc<GlyphBitmap> {
        let frame = self.frame;
        if let Some((bitmap, used)) = self.glyphs.get_mut(&key) {
            *used = frame;
            return bitmap.clone();
        }
        let mut scaler = self
            .scaler
            .builder(face.font_ref())
            .size(size)
            .hint(false)
            .build();
        let image = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .render(&mut scaler, key.glyph);
        let bitmap = Rc::new(match image {
            Some(image) => GlyphBitmap {
                key,
                width: image.placement.width,
                height: image.placement.height,
                left: image.placement.left,
                top: image.placement.top,
                coverage: image.data.into_boxed_slice(),
                atlas_slot: Cell::default(),
            },
            None => GlyphBitmap {
                key,
                width: 0,
                height: 0,
                left: 0,
                top: 0,
                coverage: Box::default(),
                atlas_slot: Cell::default(),
            },
        });
        self.glyphs.insert(key, (bitmap.clone(), frame));
        bitmap
    }

    /// Lays out `text` at `size` pixels, wrapped to `width` and aligned
    /// horizontally within it by `align`. The box height is left to the
    /// caller, so resizing a box vertically reuses the layout.
    pub fn layout(
        &mut self,
        family: Option<&str>,
        bold: bool,
        text: &str,
        size: f32,
        width: f32,
        align: TextAlign,
    ) -> Rc<TextLayout> {
        let face = self.face(family, bold);
        let face_id = face.id();
        let (size_bits, width_bits) = (size.to_bits(), width.to_bits());
        let mut hasher = FxHasher::default();
        (face_id, text, size_bits, width_bits, align as u8).hash(&mut hasher);
        let hash = hasher.finish();
        let frame = self.frame;
        if let Some(entry) = self.layouts.get_mut(&hash) {
            if entry.face == face_id
                && entry.size == size_bits
                && entry.width == width_bits
                && entry.align == align
                && &*entry.text == text
            {
                entry.used = frame;
                return entry.layout.clone();
            }
        }

        #[cfg(feature = "perf-metrics")]
        creamui_core::metrics::record(|m| m.text_layouts += 1);
        let shaped = creamui_fonts::layout(
            &face,
            text,
            size,
            &LayoutSettings {
                max_width: Some(width),
                max_height: None,
                horizontal_align: match align {
                    TextAlign::Start => HorizontalAlign::Left,
                    TextAlign::Center => HorizontalAlign::Center,
                    TextAlign::End => HorizontalAlign::Right,
                },
            },
        );

        let mut ink = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
        let mut glyphs = Vec::with_capacity(shaped.glyphs.len());
        for g in &shaped.glyphs {
            let key = GlyphKey {
                face: face_id,
                glyph: g.id,
                px: size_bits,
            };
            let bitmap = self.glyph(&face, key, size);
            if bitmap.width == 0 || bitmap.height == 0 {
                continue;
            }
            let x = g.x.round() as i32 + bitmap.left;
            let y = g.y.round() as i32 - bitmap.top;
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
        let layout = Rc::new(TextLayout {
            glyphs,
            ink,
            height: shaped.height,
        });
        self.layouts.insert(
            hash,
            LayoutEntry {
                face: face_id,
                text: text.into(),
                size: size_bits,
                width: width_bits,
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
        let a = text.layout(None, false, "Hello", 14.0, 200.0, TextAlign::Start);
        let b = text.layout(None, false, "Hello", 14.0, 200.0, TextAlign::Start);
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(a.glyphs.len(), 5);
        let c = text.layout(None, true, "Hello", 14.0, 200.0, TextAlign::Start);
        assert!(!Rc::ptr_eq(&a, &c));
        assert!(Rc::ptr_eq(&a.glyphs[2].bitmap, &a.glyphs[3].bitmap));
    }

    #[test]
    fn alignment_moves_glyphs_inside_the_box() {
        let mut text = TextSystem::new();
        let start = text.layout(None, false, "Hi", 14.0, 200.0, TextAlign::Start);
        let end = text.layout(None, false, "Hi", 14.0, 200.0, TextAlign::End);
        assert!(end.ink[0] > start.ink[0] + 100);
        assert!(end.ink[2] <= 200);
    }

    #[test]
    fn unused_entries_are_evicted() {
        let mut text = TextSystem::new();
        text.layout(None, false, "gone", 14.0, 100.0, TextAlign::Start);
        for _ in 0..EVICT_UNUSED_FOR_FRAMES + EVICT_EVERY_FRAMES {
            text.end_frame();
        }
        assert_eq!(text.cached_layouts(), 0);
        assert_eq!(text.cached_glyphs(), 0);
    }
}
