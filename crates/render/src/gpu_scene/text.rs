use bytemuck::Zeroable;
use creamui_fonts::FontWeight;
use fontdue::layout::{
    CoordinateSystem, GlyphRasterConfig, HorizontalAlign, Layout, LayoutSettings,
    TextStyle as FontdueTextStyle,
};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    pub raster_key: GlyphRasterConfig,
    pub x: f32,
    pub y: f32,
}

/// One piece of text laid out at a canonical `(0, 0)` origin with
/// `HorizontalAlign::Left` — independent of where or how it will be
/// painted, so the same shaped result serves both measurement and any
/// number of differently-aligned paints.
#[derive(Debug, Default)]
pub struct ShapedRun {
    pub glyphs: Vec<ShapedGlyph>,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ShapeKey {
    text: String,
    font_size_bits: u32,
    max_width_bits: u32,
    family: String,
    bold: bool,
}

const DEFAULT_CAPACITY: usize = 512;

struct ShapeEntry {
    run: Rc<ShapedRun>,
    last_used: u64,
}

/// An LRU cache from `(text, size, wrap width, family, weight)` to its
/// [`ShapedRun`] — REFACTOR.md 15.2/15.5's shaping cache, so unchanged text
/// is shaped once instead of on every measurement and every paint. A hit
/// costs one hashmap lookup plus a counter bump (`O(1)`, independent of how
/// much else is cached); eviction on a miss past capacity scans for the
/// least-recently-used entry, `O(capacity)` but only paid on a miss.
pub struct ShapeCache {
    entries: HashMap<ShapeKey, ShapeEntry>,
    capacity: usize,
    clock: u64,
}

impl ShapeCache {
    pub fn new() -> Self {
        ShapeCache::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        ShapeCache {
            entries: HashMap::new(),
            capacity: capacity.max(1),
            clock: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn shape(
        &mut self,
        text: &str,
        font_size: f32,
        max_width: f32,
        family: Option<&str>,
        bold: bool,
    ) -> Rc<ShapedRun> {
        let key = ShapeKey {
            text: text.to_owned(),
            font_size_bits: font_size.to_bits(),
            max_width_bits: max_width.to_bits(),
            family: family.unwrap_or(creamui_fonts::DEFAULT_FAMILY).to_owned(),
            bold,
        };
        self.clock += 1;

        if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_used = self.clock;
            return entry.run.clone();
        }

        let run = Rc::new(shape_run(text, font_size, max_width, &key.family, bold));
        self.insert(key, run.clone());
        run
    }

    fn insert(&mut self, key: ShapeKey, run: Rc<ShapedRun>) {
        if self.entries.len() >= self.capacity {
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(
            key,
            ShapeEntry {
                run,
                last_used: self.clock,
            },
        );
    }
}

impl Default for ShapeCache {
    fn default() -> Self {
        ShapeCache::new()
    }
}

fn shape_run(text: &str, font_size: f32, max_width: f32, family: &str, bold: bool) -> ShapedRun {
    let weight = if bold {
        FontWeight::Bold
    } else {
        FontWeight::Regular
    };
    let face = creamui_fonts::resolve(family, weight);

    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        max_width: Some(max_width),
        horizontal_align: HorizontalAlign::Left,
        ..LayoutSettings::default()
    });
    layout.append(&[face.as_ref()], &FontdueTextStyle::new(text, font_size, 0));

    let width = layout
        .lines()
        .and_then(|lines| lines.first())
        .map(|line| (max_width - line.padding).max(0.0))
        .unwrap_or(0.0);
    let height = layout.height().max(font_size * 1.4);

    let glyphs = layout
        .glyphs()
        .iter()
        .filter(|g| g.width > 0 && g.height > 0)
        .map(|g| ShapedGlyph {
            raster_key: g.key,
            x: g.x,
            y: g.y,
        })
        .collect();

    ShapedRun {
        glyphs,
        width: width.max(1.0),
        height,
    }
}

/// Approximates an underline/strikethrough bar for a single-line text run
/// at `(origin_x, origin_y)` — a wrapped multi-line run only gets one bar,
/// at the first line's position (see `TODO.md`).
pub fn decoration_rect(
    origin_x: f32,
    origin_y: f32,
    width: f32,
    font_size: f32,
    strikethrough: bool,
) -> creamui_core::Rect {
    let thickness = (font_size * 0.08).max(1.0);
    let offset = if strikethrough {
        font_size * 0.45
    } else {
        font_size * 0.92
    };
    creamui_core::Rect {
        x: origin_x,
        y: origin_y + offset,
        width,
        height: thickness,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

const GLYPH_PADDING: u32 = 1;

/// A shelf-packed glyph atlas mapping rasterized glyphs to a stable
/// [`AtlasRect`] within a square region of `size`.
pub struct GlyphAtlas {
    size: u32,
    cursor_x: u32,
    cursor_y: u32,
    shelf_height: u32,
    placed: HashMap<GlyphRasterConfig, AtlasRect>,
}

impl GlyphAtlas {
    pub fn new(size: u32) -> Self {
        GlyphAtlas {
            size,
            cursor_x: 0,
            cursor_y: 0,
            shelf_height: 0,
            placed: HashMap::new(),
        }
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn rect_for(&self, key: GlyphRasterConfig) -> Option<AtlasRect> {
        self.placed.get(&key).copied()
    }

    /// Places a `width` x `height` glyph bitmap for `key`, returning its
    /// rect. Returns the existing rect without moving anything if `key` is
    /// already placed. Returns `None` when the atlas has no room left — the
    /// caller should [`GlyphAtlas::grow`] and place again.
    pub fn place(&mut self, key: GlyphRasterConfig, width: u32, height: u32) -> Option<AtlasRect> {
        if let Some(existing) = self.placed.get(&key) {
            return Some(*existing);
        }

        let padded_width = width + GLYPH_PADDING;
        let padded_height = height + GLYPH_PADDING;
        if padded_width > self.size || padded_height > self.size {
            return None;
        }

        if self.cursor_x + padded_width > self.size {
            self.cursor_x = 0;
            self.cursor_y += self.shelf_height;
            self.shelf_height = 0;
        }
        if self.cursor_y + padded_height > self.size {
            return None;
        }

        let rect = AtlasRect {
            x: self.cursor_x,
            y: self.cursor_y,
            width,
            height,
        };
        self.cursor_x += padded_width;
        self.shelf_height = self.shelf_height.max(padded_height);
        self.placed.insert(key, rect);
        Some(rect)
    }

    /// Doubles `size`; existing placements stay valid, since the packer
    /// never repositions them.
    pub fn grow(&mut self) {
        self.size *= 2;
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub color: [f32; 4],
    pub clip_min: [f32; 2],
    pub clip_max: [f32; 2],
}

impl GlyphInstance {
    pub fn new(
        position: [f32; 2],
        size: [f32; 2],
        atlas_rect: AtlasRect,
        atlas_size: u32,
        color: [f32; 4],
    ) -> Self {
        let atlas_size = atlas_size as f32;
        let (clip_min, clip_max) = super::quad::UNCLIPPED;
        GlyphInstance {
            position,
            size,
            uv_min: [
                atlas_rect.x as f32 / atlas_size,
                atlas_rect.y as f32 / atlas_size,
            ],
            uv_max: [
                (atlas_rect.x + atlas_rect.width) as f32 / atlas_size,
                (atlas_rect.y + atlas_rect.height) as f32 / atlas_size,
            ],
            color,
            clip_min,
            clip_max,
        }
    }

    pub fn translated(self, dx: f32, dy: f32) -> Self {
        GlyphInstance {
            position: [self.position[0] + dx, self.position[1] + dy],
            ..self
        }
    }

    pub fn scaled_alpha(self, factor: f32) -> Self {
        let [r, g, b, a] = self.color;
        GlyphInstance {
            color: [r, g, b, a * factor],
            ..self
        }
    }

    pub fn clipped(self, clip_min: [f32; 2], clip_max: [f32; 2]) -> Self {
        GlyphInstance {
            clip_min,
            clip_max,
            ..self
        }
    }

    pub fn rescaled_uv(self, ratio: f32) -> Self {
        let [umin_x, umin_y] = self.uv_min;
        let [umax_x, umax_y] = self.uv_max;
        GlyphInstance {
            uv_min: [umin_x * ratio, umin_y * ratio],
            uv_max: [umax_x * ratio, umax_y * ratio],
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphPrimitiveId(u32);

/// Same free-list-plus-dirty-range shape as
/// [`crate::gpu_scene::QuadStore`], kept separate rather than shared
/// generic code since glyph and quad instances are unrelated data with
/// unrelated pipelines.
#[derive(Default)]
pub struct GlyphStore {
    slots: Vec<GlyphInstance>,
    free: Vec<u32>,
    dirty: Option<(u32, u32)>,
}

impl GlyphStore {
    pub fn new() -> Self {
        GlyphStore::default()
    }

    pub fn insert(&mut self, instance: GlyphInstance) -> GlyphPrimitiveId {
        let index = match self.free.pop() {
            Some(index) => {
                self.slots[index as usize] = instance;
                index
            }
            None => {
                let index = self.slots.len() as u32;
                self.slots.push(instance);
                index
            }
        };
        self.mark_dirty(index);
        GlyphPrimitiveId(index)
    }

    pub fn update(&mut self, id: GlyphPrimitiveId, instance: GlyphInstance) {
        let slot = &mut self.slots[id.0 as usize];
        if *slot != instance {
            *slot = instance;
            self.mark_dirty(id.0);
        }
    }

    pub fn remove(&mut self, id: GlyphPrimitiveId) {
        self.slots[id.0 as usize] = GlyphInstance::zeroed();
        self.free.push(id.0);
        self.mark_dirty(id.0);
    }

    pub fn get(&self, id: GlyphPrimitiveId) -> GlyphInstance {
        self.slots[id.0 as usize]
    }

    pub fn instances(&self) -> &[GlyphInstance] {
        &self.slots
    }

    pub fn take_dirty_range(&mut self) -> Option<(u32, u32)> {
        self.dirty.take()
    }

    /// Rescales every stored instance's UV by `ratio` in place and marks
    /// the whole buffer dirty — see [`GlyphInstance::rescaled_uv`].
    pub fn rescale_uv(&mut self, ratio: f32) {
        if self.slots.is_empty() {
            return;
        }
        for slot in &mut self.slots {
            *slot = slot.rescaled_uv(ratio);
        }
        self.mark_dirty(0);
        self.mark_dirty(self.slots.len() as u32 - 1);
    }

    fn mark_dirty(&mut self, index: u32) {
        self.dirty = Some(match self.dirty {
            Some((min, max)) => (min.min(index), max.max(index)),
            None => (index, index),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(glyph_index: u16) -> GlyphRasterConfig {
        GlyphRasterConfig {
            glyph_index,
            px: 16.0,
            font_hash: 0,
        }
    }

    #[test]
    fn shaping_the_same_text_twice_hits_the_cache() {
        let mut cache = ShapeCache::new();
        let a = cache.shape("hello", 16.0, 1000.0, None, false);
        let b = cache.shape("hello", 16.0, 1000.0, None, false);
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn shaping_different_text_misses_the_cache() {
        let mut cache = ShapeCache::new();
        let a = cache.shape("hello", 16.0, 1000.0, None, false);
        let b = cache.shape("world", 16.0, 1000.0, None, false);
        assert!(!Rc::ptr_eq(&a, &b));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn different_font_size_is_a_distinct_cache_entry() {
        let mut cache = ShapeCache::new();
        let a = cache.shape("hello", 16.0, 1000.0, None, false);
        let b = cache.shape("hello", 20.0, 1000.0, None, false);
        assert!(!Rc::ptr_eq(&a, &b));
    }

    #[test]
    fn eviction_drops_the_least_recently_used_entry() {
        let mut cache = ShapeCache::with_capacity(2);
        cache.shape("a", 16.0, 1000.0, None, false);
        cache.shape("b", 16.0, 1000.0, None, false);
        cache.shape("c", 16.0, 1000.0, None, false);
        assert_eq!(cache.len(), 2);

        let entries: Vec<_> = cache.entries.keys().map(|k| k.text.clone()).collect();
        assert!(!entries.contains(&"a".to_string()));
        assert!(entries.contains(&"b".to_string()));
        assert!(entries.contains(&"c".to_string()));
    }

    #[test]
    fn touching_an_entry_protects_it_from_eviction() {
        let mut cache = ShapeCache::with_capacity(2);
        cache.shape("a", 16.0, 1000.0, None, false);
        cache.shape("b", 16.0, 1000.0, None, false);
        cache.shape("a", 16.0, 1000.0, None, false);
        cache.shape("c", 16.0, 1000.0, None, false);

        let entries: Vec<_> = cache.entries.keys().map(|k| k.text.clone()).collect();
        assert!(entries.contains(&"a".to_string()));
        assert!(!entries.contains(&"b".to_string()));
    }

    #[test]
    fn wider_max_width_can_avoid_wrapping() {
        let mut cache = ShapeCache::new();
        let unwrapped = cache.shape("hello world", 16.0, 1_000_000.0, None, false);
        let wrapped = cache.shape("hello world", 16.0, 10.0, None, false);
        assert!(wrapped.height > unwrapped.height);
    }

    #[test]
    fn decoration_rect_spans_the_given_origin_and_width() {
        let rect = decoration_rect(10.0, 20.0, 50.0, 16.0, false);
        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.width, 50.0);
    }

    #[test]
    fn strikethrough_sits_above_underline() {
        let underline = decoration_rect(0.0, 0.0, 50.0, 16.0, false);
        let strikethrough = decoration_rect(0.0, 0.0, 50.0, 16.0, true);
        assert!(strikethrough.y < underline.y);
    }

    #[test]
    fn decoration_thickness_scales_with_font_size_but_has_a_floor() {
        assert_eq!(decoration_rect(0.0, 0.0, 10.0, 1.0, false).height, 1.0);
        let thick = decoration_rect(0.0, 0.0, 10.0, 100.0, false);
        assert_eq!(thick.height, 8.0);
    }

    #[test]
    fn atlas_places_distinct_glyphs_without_overlap() {
        let mut atlas = GlyphAtlas::new(64);
        let a = atlas.place(key(1), 10, 12).expect("fits");
        let b = atlas.place(key(2), 10, 12).expect("fits");
        assert_ne!(a, b);
        assert!(a.x + a.width <= b.x || b.x + b.width <= a.x || a.y + a.height <= b.y);
    }

    #[test]
    fn placing_the_same_key_twice_returns_the_same_rect() {
        let mut atlas = GlyphAtlas::new(64);
        let a = atlas.place(key(1), 10, 12).expect("fits");
        let b = atlas.place(key(1), 10, 12).expect("fits");
        assert_eq!(a, b);
    }

    #[test]
    fn atlas_reports_full_instead_of_overlapping_placements() {
        let mut atlas = GlyphAtlas::new(16);
        for i in 0..20u16 {
            atlas.place(key(i), 15, 15);
        }
        assert!(atlas.place(key(100), 15, 15).is_none());
    }

    #[test]
    fn growing_the_atlas_doubles_its_size_and_keeps_existing_placements() {
        let mut atlas = GlyphAtlas::new(16);
        let before = atlas.place(key(1), 15, 15).expect("fits");
        atlas.grow();
        assert_eq!(atlas.size(), 32);
        assert_eq!(atlas.rect_for(key(1)), Some(before));
    }

    #[test]
    fn growing_the_atlas_makes_room_for_placements_that_did_not_fit() {
        let mut atlas = GlyphAtlas::new(16);
        for i in 0..20u16 {
            atlas.place(key(i), 15, 15);
        }
        assert!(atlas.place(key(100), 15, 15).is_none());
        atlas.grow();
        assert!(atlas.place(key(100), 15, 15).is_some());
    }

    #[test]
    fn glyph_instance_uv_covers_the_atlas_rect_fraction() {
        let rect = AtlasRect {
            x: 8,
            y: 16,
            width: 4,
            height: 8,
        };
        let instance = GlyphInstance::new([0.0, 0.0], [4.0, 8.0], rect, 32, [1.0; 4]);
        assert_eq!(instance.uv_min, [0.25, 0.5]);
        assert_eq!(instance.uv_max, [0.375, 0.75]);
    }

    #[test]
    fn glyph_store_insert_marks_dirty_and_returns_distinct_ids() {
        let mut store = GlyphStore::new();
        let a = store.insert(GlyphInstance::zeroed());
        let b = store.insert(GlyphInstance::zeroed());
        assert_ne!(a, b);
        assert_eq!(store.take_dirty_range(), Some((0, 1)));
    }

    #[test]
    fn glyph_store_update_with_unchanged_instance_does_not_mark_dirty() {
        let mut store = GlyphStore::new();
        let instance = GlyphInstance::new(
            [1.0, 2.0],
            [4.0, 8.0],
            AtlasRect {
                x: 0,
                y: 0,
                width: 4,
                height: 8,
            },
            32,
            [1.0; 4],
        );
        let id = store.insert(instance);
        store.take_dirty_range();

        store.update(id, instance);
        assert_eq!(store.take_dirty_range(), None);
    }

    #[test]
    fn glyph_store_remove_reuses_the_freed_slot_on_next_insert() {
        let mut store = GlyphStore::new();
        let a = store.insert(GlyphInstance::zeroed());
        let _b = store.insert(GlyphInstance::zeroed());
        store.remove(a);

        let c = store.insert(GlyphInstance::zeroed());
        assert_eq!(c, a);
        assert_eq!(store.instances().len(), 2);
    }

    #[test]
    fn glyph_store_get_returns_the_last_inserted_or_updated_value() {
        let mut store = GlyphStore::new();
        let rect = AtlasRect {
            x: 0,
            y: 0,
            width: 4,
            height: 8,
        };
        let instance = GlyphInstance::new([1.0, 2.0], [4.0, 8.0], rect, 32, [1.0; 4]);
        let id = store.insert(instance);
        assert_eq!(store.get(id), instance);

        let moved = instance.translated(3.0, -2.0);
        store.update(id, moved);
        assert_eq!(store.get(id), moved);
    }

    #[test]
    fn glyph_translated_shifts_only_the_position() {
        let rect = AtlasRect {
            x: 0,
            y: 0,
            width: 4,
            height: 8,
        };
        let instance = GlyphInstance::new([1.0, 2.0], [4.0, 8.0], rect, 32, [1.0; 4]);
        let shifted = instance.translated(3.0, -2.0);
        assert_eq!(shifted.position, [4.0, 0.0]);
        assert_eq!(shifted.uv_min, instance.uv_min);
        assert_eq!(shifted.uv_max, instance.uv_max);
    }

    #[test]
    fn glyph_scaled_alpha_scales_only_the_color_alpha() {
        let rect = AtlasRect {
            x: 0,
            y: 0,
            width: 4,
            height: 8,
        };
        let instance = GlyphInstance::new([1.0, 2.0], [4.0, 8.0], rect, 32, [0.2, 0.4, 0.6, 0.8]);
        let scaled = instance.scaled_alpha(0.5);
        assert_eq!(scaled.color, [0.2, 0.4, 0.6, 0.4]);
        assert_eq!(scaled.position, instance.position);
    }

    #[test]
    fn glyph_clipped_sets_only_the_clip_bounds() {
        let rect = AtlasRect {
            x: 0,
            y: 0,
            width: 4,
            height: 8,
        };
        let instance = GlyphInstance::new([1.0, 2.0], [4.0, 8.0], rect, 32, [1.0; 4]);
        let clipped = instance.clipped([1.0, 2.0], [3.0, 4.0]);
        assert_eq!(clipped.clip_min, [1.0, 2.0]);
        assert_eq!(clipped.clip_max, [3.0, 4.0]);
        assert_eq!(clipped.position, instance.position);
        assert_eq!(clipped.color, instance.color);
    }

    #[test]
    fn rescaled_uv_scales_min_and_max_only() {
        let rect = AtlasRect {
            x: 8,
            y: 16,
            width: 4,
            height: 8,
        };
        let instance = GlyphInstance::new([1.0, 2.0], [4.0, 8.0], rect, 32, [1.0; 4]);
        let rescaled = instance.rescaled_uv(0.5);
        assert_eq!(rescaled.uv_min, [0.125, 0.25]);
        assert_eq!(rescaled.uv_max, [0.1875, 0.375]);
        assert_eq!(rescaled.position, instance.position);
        assert_eq!(rescaled.color, instance.color);
    }

    #[test]
    fn glyph_store_rescale_uv_updates_every_slot_and_marks_the_full_range_dirty() {
        let rect = AtlasRect {
            x: 0,
            y: 0,
            width: 4,
            height: 8,
        };
        let mut store = GlyphStore::new();
        let a = store.insert(GlyphInstance::new(
            [0.0, 0.0],
            [4.0, 8.0],
            rect,
            32,
            [1.0; 4],
        ));
        let b = store.insert(GlyphInstance::new(
            [1.0, 1.0],
            [4.0, 8.0],
            rect,
            32,
            [1.0; 4],
        ));
        store.take_dirty_range();

        store.rescale_uv(0.5);
        assert_eq!(store.take_dirty_range(), Some((0, 1)));
        assert_eq!(store.get(a).uv_min, [0.0, 0.0]);
        assert_eq!(store.get(b).uv_max, [0.0625, 0.125]);
    }

    #[test]
    fn glyph_store_rescale_uv_on_an_empty_store_does_not_mark_dirty() {
        let mut store = GlyphStore::new();
        store.rescale_uv(0.5);
        assert_eq!(store.take_dirty_range(), None);
    }
}
