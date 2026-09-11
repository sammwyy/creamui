//! [`creamui_core::Painter`] implementation backed by `tiny-skia` CPU
//! rasterization. The resulting pixmap is uploaded to a GPU texture and
//! blitted to the window surface by [`crate::gpu`] — compositing happens on
//! the GPU even though shape/glyph rasterization happens on the CPU.

use crate::font::Font;
use creamui_core::{Painter, Point, Rect, TextAlign};
use creamui_fonts::FontWeight;
use creamui_theme::{Color, ColorScheme};
use fontdue::layout::HorizontalAlign;
use std::collections::HashMap;
use std::mem;
use tiny_skia::{
    FilterQuality, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, PixmapRef, Stroke, Transform,
};

/// Saved state of whatever [`SkiaPainter`] was painting into before a
/// [`Painter::push_layer`], restored by the matching [`Painter::pop_layer`].
struct LayerFrame {
    id: u64,
    rect: Rect,
    pixmap: Pixmap,
    clip_stack: Vec<Mask>,
    origin: Point,
}

#[cfg(not(target_arch = "wasm32"))]
type PainterInstant = std::time::Instant;
#[cfg(target_arch = "wasm32")]
type PainterInstant = web_time::Instant;

/// Widgets are laid out and painted in logical (DPI-independent) pixels;
/// `SkiaPainter` scales every coordinate by `scale` (the window's
/// `scale_factor`) before rasterizing, so the backing `pixmap` — and the
/// GPU texture it's uploaded into — are always sized in physical pixels for
/// crisp output on HiDPI displays.
pub struct SkiaPainter {
    pub pixmap: Pixmap,
    font: Font,
    bold_font: Font,
    /// Resolved on first use per (family, bold), then kept for the
    /// painter's lifetime so its glyph rasterization cache stays warm.
    custom_fonts: HashMap<(String, bool), Font>,
    pub pointer: Option<Point>,
    pub press_origin: Option<Point>,
    pub animated: bool,
    /// Like `animated`, but reset by [`Painter::take_animated`] rather than
    /// [`SkiaPainter::clear`], so the renderer can attribute an
    /// `animation_time` call to the one widget that made it without
    /// disturbing the frame-level `animated` flag the window's redraw
    /// scheduler reads.
    node_animated: bool,
    started: PainterInstant,
    scale: f32,
    color_scheme: ColorScheme,
    /// One [`Mask`] per active [`Painter::push_clip`], each already
    /// intersected with its parent so the top of the stack is always the
    /// full cumulative clip region.
    clip_stack: Vec<Mask>,
    /// Popped masks, reused by [`SkiaPainter::take_mask`] instead of
    /// reallocating a window-sized buffer on every [`Painter::push_clip`].
    mask_pool: Vec<Mask>,
    /// Offset subtracted from every incoming (window-space) coordinate
    /// before rasterizing, so a [`Painter::push_layer`] can redirect drawing
    /// into a small layer-local `Pixmap` without every draw call needing to
    /// know about layers.
    origin: Point,
    layer_pool: HashMap<u64, Pixmap>,
    /// A snapshot of whatever sat behind a layer's `rect` at the last
    /// `fresh` `push_layer`, reused on every animation-only tick in between.
    /// Without this, seeding the layer's own surface from itself would
    /// re-blend each tick's content onto the *previous* tick's result
    /// instead of the true backdrop, leaking stale pixels wherever the new
    /// content doesn't happen to cover them (e.g. scrolling text).
    backdrop_pool: HashMap<u64, Pixmap>,
    layer_stack: Vec<LayerFrame>,
    /// Window-space rects touched by a layer composite since the last
    /// [`SkiaPainter::take_damage`].
    damage: Vec<Rect>,
}

impl SkiaPainter {
    /// `width`/`height` are physical pixels.
    pub fn new(width: u32, height: u32) -> Self {
        SkiaPainter {
            pixmap: Pixmap::new(width.max(1), height.max(1)).expect("non-zero pixmap size"),
            font: Font::load(),
            bold_font: Font::bold(),
            custom_fonts: HashMap::new(),
            pointer: None,
            press_origin: None,
            animated: false,
            node_animated: false,
            started: PainterInstant::now(),
            scale: 1.0,
            color_scheme: ColorScheme::default(),
            clip_stack: Vec::new(),
            mask_pool: Vec::new(),
            origin: Point::default(),
            layer_pool: HashMap::new(),
            backdrop_pool: HashMap::new(),
            layer_stack: Vec::new(),
            damage: Vec::new(),
        }
    }

    fn local(&self, rect: Rect) -> Rect {
        Rect {
            x: rect.x - self.origin.x,
            y: rect.y - self.origin.y,
            ..rect
        }
    }

    /// Copies the `phys_w` x `phys_h` region of the current paint target
    /// that a layer at `rect` sits over, for seeding that layer's surface.
    /// Out-of-bounds source pixels (partially off-window/off-layer) stay
    /// transparent, matching a freshly allocated `Pixmap`.
    fn capture_backdrop(&self, rect: Rect, phys_w: u32, phys_h: u32) -> Pixmap {
        let ox = ((rect.x - self.origin.x) * self.scale).round() as i64;
        let oy = ((rect.y - self.origin.y) * self.scale).round() as i64;
        let src_w = self.pixmap.width() as i64;
        let src_h = self.pixmap.height() as i64;
        let src = self.pixmap.pixels();
        let mut backdrop = Pixmap::new(phys_w, phys_h).expect("non-zero pixmap size");
        let dst = backdrop.pixels_mut();
        for ly in 0..phys_h as i64 {
            let py = oy + ly;
            if py < 0 || py >= src_h {
                continue;
            }
            for lx in 0..phys_w as i64 {
                let px = ox + lx;
                if px < 0 || px >= src_w {
                    continue;
                }
                dst[(ly * phys_w as i64 + lx) as usize] = src[(py * src_w + px) as usize];
            }
        }
        backdrop
    }

    /// A zero-filled, `width` x `height` [`Mask`], reusing a pooled one when
    /// possible.
    fn take_mask(&mut self, width: u32, height: u32) -> Mask {
        while let Some(mut mask) = self.mask_pool.pop() {
            if mask.width() == width && mask.height() == height {
                mask.clear();
                return mask;
            }
        }
        Mask::new(width, height).expect("non-zero pixmap size")
    }

    /// Sets the logical-to-physical pixel scale factor applied to every
    /// subsequent paint call.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.max(0.01);
    }

    pub fn set_color_scheme(&mut self, color_scheme: ColorScheme) {
        self.color_scheme = color_scheme;
    }

    /// `width`/`height` are physical pixels.
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        // Reallocating a full window-sized pixel buffer for every reactive
        // frame dominated pointer-drag time. Most frames are not resizes;
        // retain and clear the existing buffer in that overwhelmingly common
        // case.
        if self.pixmap.width() == width && self.pixmap.height() == height {
            return;
        }
        self.pixmap = Pixmap::new(width, height).expect("non-zero pixmap size");
        // A resize mid-clip-stack shouldn't happen (push/pop are balanced
        // within one frame, and resize only ever runs between frames), but
        // clear defensively rather than risk stale masks sized for the old pixmap.
        self.clip_stack.clear();
        self.mask_pool.clear();
        self.layer_stack.clear();
        self.origin = Point::default();
    }

    pub fn clear(&mut self, color: Color) {
        self.animated = false;
        let [r, g, b, a] = color.to_f32();
        self.pixmap
            .fill(tiny_skia::Color::from_rgba(r, g, b, a).expect("valid color"));
    }

    /// Resolves the [`Font`] wrapper for `family`/`bold`, creating and
    /// caching one on first use. `None` uses the bundled default.
    fn font_for(&mut self, family: Option<&str>, bold: bool) -> &mut Font {
        match family {
            None => {
                if bold {
                    &mut self.bold_font
                } else {
                    &mut self.font
                }
            }
            Some(family) => self
                .custom_fonts
                .entry((family.to_string(), bold))
                .or_insert_with(|| {
                    let weight = if bold {
                        FontWeight::Bold
                    } else {
                        FontWeight::Regular
                    };
                    Font::from_family(family, weight)
                }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text(
        &mut self,
        rect: Rect,
        text: &str,
        color: Color,
        selected: Option<(std::ops::Range<usize>, Color)>,
        font_size: f32,
        align: TextAlign,
        family: Option<&str>,
        bold: bool,
        italic: bool,
    ) {
        let rect = self.local(rect);
        let horizontal_align = match align {
            TextAlign::Start => HorizontalAlign::Left,
            TextAlign::Center => HorizontalAlign::Center,
            TextAlign::End => HorizontalAlign::Right,
        };
        let scale = self.scale;
        let rect = scale_rect(rect, scale);
        let font = self.font_for(family, bold);
        let glyphs = font.layout_text(
            text,
            font_size * scale,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            horizontal_align,
        );
        let pixmap_width = self.pixmap.width() as i32;
        let pixmap_height = self.pixmap.height() as i32;
        let clip_mask = self.clip_stack.last();
        let pixels = self.pixmap.pixels_mut();
        // No italic face is bundled, so italics are synthesized by shearing
        // each glyph's rows rightward the further they sit above its
        // baseline (its own bottom row) — a cheap oblique that avoids
        // shipping and layout-matching a second font file.
        const ITALIC_SHEAR: f32 = 0.22;
        for glyph in glyphs {
            let glyph_color = selected
                .as_ref()
                .filter(|(range, _)| range.contains(&glyph.byte_offset))
                .map_or(color, |(_, selected_color)| *selected_color);
            let text_alpha = glyph_color.a as f32 / 255.0;
            for gy in 0..glyph.height {
                let py = glyph.y + gy as i32;
                if py < 0 || py >= pixmap_height {
                    continue;
                }
                let shear = if italic {
                    ((glyph.height as i32 - 1 - gy as i32) as f32 * ITALIC_SHEAR).round() as i32
                } else {
                    0
                };
                for gx in 0..glyph.width {
                    let px = glyph.x + gx as i32 + shear;
                    if px < 0 || px >= pixmap_width {
                        continue;
                    }
                    let coverage = glyph.coverage[gy * glyph.width + gx] as f32 / 255.0;
                    let idx = (py * pixmap_width + px) as usize;
                    let clip = clip_mask
                        .map(|mask| mask.data()[idx] as f32 / 255.0)
                        .unwrap_or(1.0);
                    let src_alpha = coverage * text_alpha * clip;
                    if src_alpha <= 0.0 {
                        continue;
                    }
                    let dst = pixels[idx];
                    let inv = 1.0 - src_alpha;
                    let out_r = (glyph_color.r as f32 * src_alpha) + (dst.red() as f32 * inv);
                    let out_g = (glyph_color.g as f32 * src_alpha) + (dst.green() as f32 * inv);
                    let out_b = (glyph_color.b as f32 * src_alpha) + (dst.blue() as f32 * inv);
                    let out_a = (255.0 * src_alpha) + (dst.alpha() as f32 * inv);
                    pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(
                        out_r.round() as u8,
                        out_g.round() as u8,
                        out_b.round() as u8,
                        out_a.round() as u8,
                    )
                    .unwrap_or(dst);
                }
            }
        }
    }

    fn rounded_rect_path(rect: Rect, radius: f32) -> Option<tiny_skia::Path> {
        let radius = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
        let mut pb = PathBuilder::new();
        let (x, y, w, h) = (rect.x, rect.y, rect.width, rect.height);
        if radius <= 0.01 {
            pb.push_rect(tiny_skia::Rect::from_xywh(x, y, w, h)?);
        } else {
            const K: f32 = 0.5522847498;
            let r = radius;
            pb.move_to(x + r, y);
            pb.line_to(x + w - r, y);
            pb.cubic_to(x + w - r + r * K, y, x + w, y + r - r * K, x + w, y + r);
            pb.line_to(x + w, y + h - r);
            pb.cubic_to(
                x + w,
                y + h - r + r * K,
                x + w - r + r * K,
                y + h,
                x + w - r,
                y + h,
            );
            pb.line_to(x + r, y + h);
            pb.cubic_to(x + r - r * K, y + h, x, y + h - r + r * K, x, y + h - r);
            pb.line_to(x, y + r);
            pb.cubic_to(x, y + r - r * K, x + r - r * K, y, x + r, y);
            pb.close();
        }
        pb.finish()
    }
}

fn scale_rect(rect: Rect, scale: f32) -> Rect {
    Rect {
        x: rect.x * scale,
        y: rect.y * scale,
        width: rect.width * scale,
        height: rect.height * scale,
    }
}

impl Painter for SkiaPainter {
    fn color_scheme(&self) -> ColorScheme {
        self.color_scheme
    }

    fn hovered(&self, rect: Rect) -> bool {
        self.pointer.is_some_and(|point| rect.contains(point))
    }
    fn pressed(&self, rect: Rect) -> bool {
        self.hovered(rect) && self.press_origin.is_some_and(|point| rect.contains(point))
    }
    fn animation_time(&mut self) -> f32 {
        self.animated = true;
        self.node_animated = true;
        self.started.elapsed().as_secs_f32()
    }
    fn take_animated(&mut self) -> bool {
        mem::take(&mut self.node_animated)
    }
    fn take_damage(&mut self) -> Vec<Rect> {
        mem::take(&mut self.damage)
    }
    fn begin_animated_frame(&mut self) {
        self.animated = false;
    }
    fn push_layer(&mut self, id: u64, rect: Rect, fresh: bool) {
        let scale = self.scale;
        let phys_w = ((rect.width * scale).round() as u32).max(1);
        let phys_h = ((rect.height * scale).round() as u32).max(1);

        let cached_backdrop_matches = matches!(
            self.backdrop_pool.get(&id),
            Some(p) if p.width() == phys_w && p.height() == phys_h
        );
        let backdrop = if fresh || !cached_backdrop_matches {
            self.capture_backdrop(rect, phys_w, phys_h)
        } else {
            self.backdrop_pool.remove(&id).expect("checked above")
        };

        let mut layer_pixmap = match self.layer_pool.remove(&id) {
            Some(pixmap) if pixmap.width() == phys_w && pixmap.height() == phys_h => pixmap,
            _ => Pixmap::new(phys_w, phys_h).expect("non-zero pixmap size"),
        };
        layer_pixmap.data_mut().copy_from_slice(backdrop.data());
        self.backdrop_pool.insert(id, backdrop);

        let mut layer_clip_stack = Vec::new();
        if let Some(parent_mask) = self.clip_stack.last() {
            let ox = ((rect.x - self.origin.x) * scale).round() as i64;
            let oy = ((rect.y - self.origin.y) * scale).round() as i64;
            let parent_w = parent_mask.width() as i64;
            let parent_h = parent_mask.height() as i64;
            let mut mask = Mask::new(phys_w, phys_h).expect("non-zero pixmap size");
            for ly in 0..phys_h as i64 {
                let py = oy + ly;
                for lx in 0..phys_w as i64 {
                    let px = ox + lx;
                    let v = if px >= 0 && py >= 0 && px < parent_w && py < parent_h {
                        parent_mask.data()[(py * parent_w + px) as usize]
                    } else {
                        0
                    };
                    mask.data_mut()[(ly * phys_w as i64 + lx) as usize] = v;
                }
            }
            layer_clip_stack.push(mask);
        }

        let saved_pixmap = mem::replace(&mut self.pixmap, layer_pixmap);
        let saved_clip_stack = mem::replace(&mut self.clip_stack, layer_clip_stack);
        let saved_origin = self.origin;
        self.origin = Point {
            x: rect.x,
            y: rect.y,
        };
        self.damage.push(rect);
        self.layer_stack.push(LayerFrame {
            id,
            rect,
            pixmap: saved_pixmap,
            clip_stack: saved_clip_stack,
            origin: saved_origin,
        });
    }
    fn pop_layer(&mut self) {
        let Some(frame) = self.layer_stack.pop() else {
            return;
        };
        let finished = mem::replace(&mut self.pixmap, frame.pixmap);
        self.clip_stack = frame.clip_stack;
        self.origin = frame.origin;
        let x = ((frame.rect.x - self.origin.x) * self.scale).round() as i32;
        let y = ((frame.rect.y - self.origin.y) * self.scale).round() as i32;
        // `Source`, not the default `SourceOver`: the layer was seeded from
        // its own backdrop before painting, so it's a complete snapshot of
        // this rect — a transparent layer pixel means "the backdrop was
        // transparent here", not "leave whatever's already composited".
        // Blending would leave stale content wherever this tick's paint
        // doesn't happen to re-cover what the last tick drew.
        let composite = PixmapPaint {
            blend_mode: tiny_skia::BlendMode::Source,
            ..PixmapPaint::default()
        };
        self.pixmap.draw_pixmap(
            x,
            y,
            finished.as_ref(),
            &composite,
            Transform::identity(),
            self.clip_stack.last(),
        );
        self.layer_pool.insert(frame.id, finished);
    }
    fn forget_layer(&mut self, id: u64) {
        self.layer_pool.remove(&id);
        self.backdrop_pool.remove(&id);
    }
    fn composite_cached_layer(&mut self, id: u64, rect: Rect) -> bool {
        let scale = self.scale;
        let phys_w = ((rect.width * scale).round() as u32).max(1);
        let phys_h = ((rect.height * scale).round() as u32).max(1);
        let Some(cached) = self.layer_pool.get(&id) else {
            return false;
        };
        if cached.width() != phys_w || cached.height() != phys_h {
            return false;
        }
        let x = ((rect.x - self.origin.x) * scale).round() as i32;
        let y = ((rect.y - self.origin.y) * scale).round() as i32;
        let composite = PixmapPaint {
            blend_mode: tiny_skia::BlendMode::Source,
            ..PixmapPaint::default()
        };
        self.pixmap.draw_pixmap(
            x,
            y,
            cached.as_ref(),
            &composite,
            Transform::identity(),
            self.clip_stack.last(),
        );
        true
    }
    fn stroke_line(&mut self, from: Point, to: Point, color: Color, width: f32) {
        let from = Point {
            x: from.x - self.origin.x,
            y: from.y - self.origin.y,
        };
        let to = Point {
            x: to.x - self.origin.x,
            y: to.y - self.origin.y,
        };
        let mut path = PathBuilder::new();
        path.move_to(from.x * self.scale, from.y * self.scale);
        path.line_to(to.x * self.scale, to.y * self.scale);
        if let Some(path) = path.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(color.r, color.g, color.b, color.a);
            let stroke = Stroke {
                width: width * self.scale,
                line_cap: tiny_skia::LineCap::Round,
                ..Default::default()
            };
            self.pixmap.stroke_path(
                &path,
                &paint,
                &stroke,
                Transform::identity(),
                self.clip_stack.last(),
            );
        }
    }
    fn fill_text_weight(
        &mut self,
        rect: Rect,
        text: &str,
        color: Color,
        font_size: f32,
        align: TextAlign,
        bold: bool,
        italic: bool,
    ) {
        self.draw_text(
            rect, text, color, None, font_size, align, None, bold, italic,
        );
    }

    fn fill_text_font(
        &mut self,
        rect: Rect,
        text: &str,
        color: Color,
        font_size: f32,
        align: TextAlign,
        family: Option<&str>,
        bold: bool,
        italic: bool,
    ) {
        self.draw_text(
            rect, text, color, None, font_size, align, family, bold, italic,
        );
    }

    fn fill_rect(&mut self, rect: Rect, color: Color, corner_radius: f32) {
        let rect = self.local(rect);
        let Some(path) =
            Self::rounded_rect_path(scale_rect(rect, self.scale), corner_radius * self.scale)
        else {
            return;
        };
        let [r, g, b, a] = color.to_f32();
        let mut paint = Paint::default();
        paint.set_color_rgba8(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            (a * 255.0) as u8,
        );
        paint.anti_alias = true;
        self.pixmap.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            self.clip_stack.last(),
        );
    }

    fn draw_rgba_image(&mut self, rect: Rect, pixels: &[u8], width: u32, height: u32) {
        if width == 0 || height == 0 || rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }
        let Some(source) = PixmapRef::from_bytes(pixels, width, height) else {
            return;
        };
        let rect = scale_rect(self.local(rect), self.scale);
        let transform = Transform::from_row(
            rect.width / width as f32,
            0.0,
            0.0,
            rect.height / height as f32,
            rect.x,
            rect.y,
        );
        let paint = PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..Default::default()
        };
        self.pixmap
            .draw_pixmap(0, 0, source, &paint, transform, self.clip_stack.last());
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32, corner_radius: f32) {
        let rect = self.local(rect);
        let Some(path) =
            Self::rounded_rect_path(scale_rect(rect, self.scale), corner_radius * self.scale)
        else {
            return;
        };
        let [r, g, b, a] = color.to_f32();
        let mut paint = Paint::default();
        paint.set_color_rgba8(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            (a * 255.0) as u8,
        );
        paint.anti_alias = true;
        let stroke = Stroke {
            width: width * self.scale,
            ..Default::default()
        };
        self.pixmap.stroke_path(
            &path,
            &paint,
            &stroke,
            Transform::identity(),
            self.clip_stack.last(),
        );
    }

    fn push_clip(&mut self, rect: Rect) {
        self.push_clip_rounded(rect, 0.0);
    }

    fn push_clip_rounded(&mut self, rect: Rect, corner_radius: f32) {
        let width = self.pixmap.width();
        let height = self.pixmap.height();
        let scaled = scale_rect(self.local(rect), self.scale);

        let Some(path) = Self::rounded_rect_path(scaled, corner_radius * self.scale) else {
            // Degenerate (zero-size) clip rect: nothing inside it can be
            // visible, so push a fully-blocking (all-zero) mask.
            let mask = self.take_mask(width, height);
            self.clip_stack.push(mask);
            return;
        };

        let mut mask = self.take_mask(width, height);
        match self.clip_stack.last() {
            Some(parent) => {
                mask.data_mut().copy_from_slice(parent.data());
                mask.intersect_path(
                    &path,
                    tiny_skia::FillRule::Winding,
                    true,
                    Transform::identity(),
                );
            }
            None => {
                mask.fill_path(
                    &path,
                    tiny_skia::FillRule::Winding,
                    true,
                    Transform::identity(),
                );
            }
        }
        self.clip_stack.push(mask);
    }

    fn pop_clip(&mut self) {
        if let Some(mask) = self.clip_stack.pop() {
            self.mask_pool.push(mask);
        }
    }

    fn fill_text(
        &mut self,
        rect: Rect,
        text: &str,
        color: Color,
        font_size: f32,
        align: TextAlign,
    ) {
        self.draw_text(
            rect, text, color, None, font_size, align, None, false, false,
        );
    }

    fn fill_text_selected(
        &mut self,
        rect: Rect,
        text: &str,
        color: Color,
        selected_color: Color,
        selected: std::ops::Range<usize>,
        font_size: f32,
        align: TextAlign,
    ) {
        self.draw_text(
            rect,
            text,
            color,
            Some((selected, selected_color)),
            font_size,
            align,
            None,
            false,
            false,
        );
    }

    fn fill_text_selected_font(
        &mut self,
        rect: Rect,
        text: &str,
        color: Color,
        selected_color: Color,
        selected: std::ops::Range<usize>,
        font_size: f32,
        align: TextAlign,
        family: Option<&str>,
    ) {
        self.draw_text(
            rect,
            text,
            color,
            Some((selected, selected_color)),
            font_size,
            align,
            family,
            false,
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_rgba_images_at_the_requested_destination() {
        let mut painter = SkiaPainter::new(8, 8);
        painter.clear(Color::rgba(0, 0, 0, 0));
        painter.draw_rgba_image(
            Rect {
                x: 2.0,
                y: 1.0,
                width: 4.0,
                height: 4.0,
            },
            &[255, 0, 0, 255],
            1,
            1,
        );
        assert_eq!(painter.pixmap.pixel(3, 2).unwrap().red(), 255);
        assert_eq!(painter.pixmap.pixel(0, 0).unwrap().alpha(), 0);
    }

    #[test]
    fn fill_text_font_caches_one_font_per_family_and_weight() {
        let mut painter = SkiaPainter::new(64, 64);
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 64.0,
            height: 64.0,
        };
        painter.fill_text_font(
            rect,
            "Hi",
            Color::rgb(255, 255, 255),
            12.0,
            TextAlign::Start,
            Some("Custom Family"),
            false,
            false,
        );
        painter.fill_text_font(
            rect,
            "Hi",
            Color::rgb(255, 255, 255),
            12.0,
            TextAlign::Start,
            Some("Custom Family"),
            true,
            false,
        );
        assert_eq!(painter.custom_fonts.len(), 2);
        assert!(painter
            .custom_fonts
            .contains_key(&("Custom Family".to_string(), false)));
        assert!(painter
            .custom_fonts
            .contains_key(&("Custom Family".to_string(), true)));
    }

    use std::cell::Cell;
    use std::rc::Rc;

    struct HalfSplit {
        left: Rc<Cell<bool>>,
    }
    impl creamui_core::Widget for HalfSplit {
        fn style(&self) -> creamui_core::Style {
            creamui_core::Style::new().layout(creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(20.0),
                    height: creamui_core::layout::Dimension::Length(10.0),
                },
                ..Default::default()
            })
        }
        fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
            painter.animation_time();
            let half = Rect {
                x: if self.left.get() {
                    rect.x
                } else {
                    rect.x + rect.width / 2.0
                },
                y: rect.y,
                width: rect.width / 2.0,
                height: rect.height,
            };
            painter.fill_rect(half, Color::rgb(255, 0, 0), 0.0);
        }
    }

    struct CachedContainer {
        child: Option<creamui_core::BoxedWidget>,
    }
    impl creamui_core::Widget for CachedContainer {
        fn style(&self) -> creamui_core::Style {
            creamui_core::Style::new().layout(creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(20.0),
                    height: creamui_core::layout::Dimension::Length(10.0),
                },
                ..Default::default()
            })
        }
        fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
            painter.fill_rect(rect, Color::rgb(0, 0, 255), 0.0);
        }
        fn paint_fingerprint(&self) -> Option<u64> {
            Some(1)
        }
        fn children(&mut self) -> Vec<creamui_core::BoxedWidget> {
            self.child.take().into_iter().collect()
        }
    }

    #[test]
    fn promoted_layer_does_not_leak_the_other_ticks_content() {
        use creamui_core::Renderer;

        let left = Rc::new(Cell::new(true));
        let build =
            |left: Rc<Cell<bool>>| -> creamui_core::BoxedWidget { Box::new(HalfSplit { left }) };
        let viewport = creamui_core::Size {
            width: 20.0,
            height: 10.0,
        };

        let mut renderer = Renderer::new();
        let mut painter = SkiaPainter::new(20, 10);

        // Two full passes with a `clear()` between them, matching a real
        // window's render loop — the second one is where the promotion
        // streak crosses the threshold and starts using a layer.
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(build(left.clone()), viewport, &mut painter);
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(build(left.clone()), viewport, &mut painter);
        assert_eq!(painter.pixmap.pixel(4, 5).unwrap().red(), 255);
        assert_eq!(painter.pixmap.pixel(15, 5).unwrap().alpha(), 0);

        left.set(false);
        renderer.repaint_animated(&mut painter, None, false);
        assert_eq!(
            painter.pixmap.pixel(15, 5).unwrap().red(),
            255,
            "the newly painted half should show up"
        );
        assert_eq!(
            painter.pixmap.pixel(4, 5).unwrap().alpha(),
            0,
            "the half this tick didn't paint must revert to the backdrop, \
             not keep showing the other tick's content"
        );

        left.set(true);
        renderer.repaint_animated(&mut painter, None, false);
        assert_eq!(painter.pixmap.pixel(4, 5).unwrap().red(), 255);
        assert_eq!(
            painter.pixmap.pixel(15, 5).unwrap().alpha(),
            0,
            "switching back must not leave the previous tick's half behind"
        );
    }

    #[test]
    fn cached_parent_does_not_freeze_an_animated_childs_previous_frame() {
        use creamui_core::Renderer;

        let left = Rc::new(Cell::new(true));
        let build = |left: Rc<Cell<bool>>| {
            Box::new(CachedContainer {
                child: Some(Box::new(HalfSplit { left })),
            }) as creamui_core::BoxedWidget
        };
        let viewport = creamui_core::Size {
            width: 20.0,
            height: 10.0,
        };
        let mut renderer = Renderer::new();
        let mut painter = SkiaPainter::new(20, 10);

        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(build(left.clone()), viewport, &mut painter);
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(build(left.clone()), viewport, &mut painter);

        left.set(false);
        renderer.repaint_animated(&mut painter, None, false);

        assert_eq!(painter.pixmap.pixel(4, 5).unwrap().blue(), 255);
        assert_eq!(painter.pixmap.pixel(15, 5).unwrap().red(), 255);
    }

    #[test]
    fn push_layer_with_no_cached_backdrop_falls_back_to_capturing_one() {
        let mut painter = SkiaPainter::new(8, 8);
        painter.clear(Color::rgba(10, 20, 30, 255));
        // `fresh: false` with nothing cached yet for this id — the
        // defensive path `push_layer` must still take instead of seeding
        // from garbage.
        painter.push_layer(
            1,
            Rect {
                x: 2.0,
                y: 2.0,
                width: 4.0,
                height: 4.0,
            },
            false,
        );
        painter.fill_rect(
            Rect {
                x: 2.0,
                y: 2.0,
                width: 2.0,
                height: 4.0,
            },
            Color::rgb(255, 0, 0),
            0.0,
        );
        painter.pop_layer();
        assert_eq!(painter.pixmap.pixel(3, 3).unwrap().red(), 255);
        // The untouched half of the layer came from the captured backdrop
        // (the clear color), not a transparent hole.
        let untouched = painter.pixmap.pixel(5, 3).unwrap();
        assert_eq!(
            (untouched.red(), untouched.green(), untouched.blue()),
            (10, 20, 30)
        );
    }

    struct HoverAware {
        fingerprint: u64,
        paints: Rc<Cell<usize>>,
    }
    impl creamui_core::Widget for HoverAware {
        fn style(&self) -> creamui_core::Style {
            creamui_core::Style::new().layout(creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(20.0),
                    height: creamui_core::layout::Dimension::Length(10.0),
                },
                ..Default::default()
            })
        }
        fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
            self.paints.set(self.paints.get() + 1);
            let color = if painter.hovered(rect) {
                Color::rgb(255, 0, 0)
            } else {
                Color::rgb(0, 0, 255)
            };
            painter.fill_rect(rect, color, 0.0);
        }
        fn paint_fingerprint(&self) -> Option<u64> {
            Some(self.fingerprint)
        }
    }

    #[test]
    fn fingerprint_cache_does_not_hide_a_hover_change() {
        use creamui_core::Renderer;

        let paints = Rc::new(Cell::new(0usize));
        let build = |paints: Rc<Cell<usize>>| -> creamui_core::BoxedWidget {
            Box::new(HoverAware {
                fingerprint: 1,
                paints,
            })
        };
        let viewport = creamui_core::Size {
            width: 20.0,
            height: 10.0,
        };

        let mut renderer = Renderer::new();
        let mut painter = SkiaPainter::new(20, 10);

        painter.pointer = None;
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(build(paints.clone()), viewport, &mut painter);
        assert_eq!(painter.pixmap.pixel(10, 5).unwrap().blue(), 255);
        assert_eq!(paints.get(), 1);

        // Same fingerprint, same (unhovered) state: an unrelated rebuild
        // should reuse the cached layer rather than repaint.
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(build(paints.clone()), viewport, &mut painter);
        assert_eq!(
            paints.get(),
            1,
            "an unchanged fingerprint and state should hit the cache"
        );
        assert_eq!(painter.pixmap.pixel(10, 5).unwrap().blue(), 255);

        // Same fingerprint, but now hovered: the cache must not hide this.
        painter.pointer = Some(Point { x: 10.0, y: 5.0 });
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(build(paints.clone()), viewport, &mut painter);
        assert_eq!(
            paints.get(),
            2,
            "a hover-driven appearance change must invalidate the cache despite a matching fingerprint"
        );
        assert_eq!(painter.pixmap.pixel(10, 5).unwrap().red(), 255);
    }

    #[test]
    fn raw_button_hover_background_is_not_hidden_by_the_fingerprint_cache() {
        use creamui_core::{PaintStyle, Renderer, StateStyle, Style};
        use creamui_widgets::RawButton;

        let style = Style::new()
            .layout(creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(20.0),
                    height: creamui_core::layout::Dimension::Length(10.0),
                },
                ..Default::default()
            })
            .background(Color::rgb(0, 0, 255))
            .hover(StateStyle {
                paint: PaintStyle {
                    background: Some(Color::rgb(255, 0, 0).into()),
                    ..Default::default()
                },
                ..Default::default()
            });

        let viewport = creamui_core::Size {
            width: 20.0,
            height: 10.0,
        };
        let mut renderer = Renderer::new();
        let mut painter = SkiaPainter::new(20, 10);

        painter.pointer = None;
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(
            Box::new(RawButton::new(style.clone(), || {})),
            viewport,
            &mut painter,
        );
        assert_eq!(painter.pixmap.pixel(10, 5).unwrap().blue(), 255);

        // Same fingerprint, still unhovered: an unrelated rebuild should hit
        // the cache instead of repainting.
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(
            Box::new(RawButton::new(style.clone(), || {})),
            viewport,
            &mut painter,
        );
        assert_eq!(painter.pixmap.pixel(10, 5).unwrap().blue(), 255);

        // Same fingerprint, now hovered: the cache must not hide this.
        painter.pointer = Some(Point { x: 10.0, y: 5.0 });
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(
            Box::new(RawButton::new(style, || {})),
            viewport,
            &mut painter,
        );
        assert_eq!(
            painter.pixmap.pixel(10, 5).unwrap().red(),
            255,
            "a hover-driven background change must invalidate the cache despite a matching fingerprint"
        );
    }

    struct ReproLeaf {
        tag: String,
        color: Color,
    }
    impl creamui_core::Widget for ReproLeaf {
        fn style(&self) -> creamui_core::Style {
            creamui_core::Style::new().layout(creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(20.0),
                    height: creamui_core::layout::Dimension::Length(10.0),
                },
                ..Default::default()
            })
        }
        fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
            painter.fill_rect(rect, self.color, 0.0);
        }
        fn paint_fingerprint(&self) -> Option<u64> {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            self.tag.hash(&mut hasher);
            Some(hasher.finish())
        }
    }

    struct ReproContainer {
        children: Vec<creamui_core::BoxedWidget>,
        column: bool,
        width: f32,
        height: f32,
    }
    impl creamui_core::Widget for ReproContainer {
        fn style(&self) -> creamui_core::Style {
            creamui_core::Style::new().layout(creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(self.width),
                    height: creamui_core::layout::Dimension::Length(self.height),
                },
                flex_direction: if self.column {
                    creamui_core::layout::FlexDirection::Column
                } else {
                    creamui_core::layout::FlexDirection::Row
                },
                ..Default::default()
            })
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn children(&mut self) -> Vec<creamui_core::BoxedWidget> {
            std::mem::take(&mut self.children)
        }
        // Deliberately no `paint_fingerprint` override, matching
        // `creamui_widgets::layout::Flex`/generic containers: never cached.
    }

    fn repro_card(name_tag: String, icon_has_fingerprint: bool) -> creamui_core::BoxedWidget {
        let icon: creamui_core::BoxedWidget = if icon_has_fingerprint {
            Box::new(ReproLeaf {
                tag: "icon".to_string(),
                color: Color::rgb(0, 255, 0),
            })
        } else {
            Box::new(ReproContainer {
                children: Vec::new(),
                column: false,
                width: 20.0,
                height: 10.0,
            })
        };
        Box::new(ReproContainer {
            children: vec![
                icon,
                Box::new(ReproLeaf {
                    tag: name_tag,
                    color: Color::rgb(255, 0, 255),
                }),
            ],
            column: true,
            width: 20.0,
            height: 20.0,
        })
    }

    #[test]
    fn positional_reconcile_across_a_root_type_change_does_not_blank_out_labels() {
        use creamui_core::Renderer;

        const N: usize = 140;
        let mut renderer = Renderer::new();
        let mut painter = SkiaPainter::new(20 * N as u32, 20);
        let viewport = creamui_core::Size {
            width: 20.0 * N as f32,
            height: 20.0,
        };

        // Render 1: a single "Loading…" leaf at the root — matches
        // `app_drawer::build()`'s `apps.is_empty()` branch.
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(
            Box::new(ReproLeaf {
                tag: "loading".to_string(),
                color: Color::rgb(255, 255, 0),
            }),
            viewport,
            &mut painter,
        );

        // Render 2: the root becomes a row of N cards, each with an
        // icon slot (no fingerprint, matching a fallback-avatar `Flex`)
        // and a name label — matches the grid appearing once the catalog
        // loads.
        let build_grid = |icon_has_fingerprint: bool| -> creamui_core::BoxedWidget {
            Box::new(ReproContainer {
                children: (0..N)
                    .map(|i| repro_card(format!("name-{i}"), icon_has_fingerprint))
                    .collect(),
                column: false,
                width: 20.0 * N as f32,
                height: 20.0,
            })
        };
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(build_grid(false), viewport, &mut painter);

        // Render 3: icons resolve — each icon slot switches from the
        // no-fingerprint container to a fingerprinted leaf, matching the
        // fallback-avatar-`Flex` -> `Image` swap in `app_card()`. Card
        // structure and every name label are otherwise unchanged.
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(build_grid(true), viewport, &mut painter);

        let mut blank = Vec::new();
        for i in 0..N {
            let x = (i as u32) * 20 + 10;
            let y = 15;
            let pixel = painter.pixmap.pixel(x, y).unwrap();
            if (pixel.red(), pixel.green(), pixel.blue()) != (255, 0, 255) {
                blank.push(i);
            }
        }
        assert!(
            blank.is_empty(),
            "{} of {N} name labels did not render their expected color at their \
             own position after the icon-slot type swap: {blank:?}",
            blank.len()
        );
    }

    struct ClipAncestor {
        width: f32,
        child: Option<creamui_core::BoxedWidget>,
    }
    impl creamui_core::Widget for ClipAncestor {
        fn style(&self) -> creamui_core::Style {
            creamui_core::Style::new().layout(creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(self.width),
                    height: creamui_core::layout::Dimension::Length(10.0),
                },
                ..Default::default()
            })
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn children(&mut self) -> Vec<creamui_core::BoxedWidget> {
            self.child.take().into_iter().collect()
        }
        fn clips_children(&self) -> bool {
            true
        }
    }

    struct CountingLeaf {
        tag: String,
        color: Color,
        paints: Rc<Cell<usize>>,
    }
    impl creamui_core::Widget for CountingLeaf {
        fn style(&self) -> creamui_core::Style {
            creamui_core::Style::new().layout(creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(20.0),
                    height: creamui_core::layout::Dimension::Length(10.0),
                },
                // Matches real scrollable content: keeps its own size
                // instead of shrinking to fit a narrower ancestor, so only
                // the paint-time clip (not layout) differs between renders.
                flex_shrink: 0.0,
                ..Default::default()
            })
        }
        fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
            self.paints.set(self.paints.get() + 1);
            painter.fill_rect(rect, self.color, 0.0);
        }
        fn paint_fingerprint(&self) -> Option<u64> {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            self.tag.hash(&mut hasher);
            Some(hasher.finish())
        }
    }

    #[test]
    fn fingerprint_cache_seeded_under_a_narrow_clip_stays_wrong_after_the_clip_widens() {
        use creamui_core::Renderer;

        let paints = Rc::new(Cell::new(0usize));
        let leaf = |paints: Rc<Cell<usize>>| -> creamui_core::BoxedWidget {
            Box::new(CountingLeaf {
                tag: "leaf".to_string(),
                color: Color::rgb(255, 0, 255),
                paints,
            })
        };
        let viewport = creamui_core::Size {
            width: 20.0,
            height: 10.0,
        };
        let mut renderer = Renderer::new();
        let mut painter = SkiaPainter::new(20, 10);

        // First-ever paint happens under a narrow ancestor clip (only the
        // leaf's first 5 physical columns are visible) — the leaf's
        // fingerprint alone promotes it to a layer on this very paint.
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(
            Box::new(ClipAncestor {
                width: 5.0,
                child: Some(leaf(paints.clone())),
            }),
            viewport,
            &mut painter,
        );
        assert_eq!(painter.pixmap.pixel(2, 5).unwrap().red(), 255);
        assert_eq!(
            painter.pixmap.pixel(15, 5).unwrap().alpha(),
            0,
            "sanity check: the narrow clip hides the rest on the first paint"
        );
        assert_eq!(paints.get(), 1);

        // The ancestor's clip widens to the leaf's full width. Same
        // fingerprint, same (unhovered) state — this is exactly the
        // situation `paint_instance` treats as a cache hit.
        painter.clear(Color::rgba(0, 0, 0, 0));
        renderer.render(
            Box::new(ClipAncestor {
                width: 20.0,
                child: Some(leaf(paints.clone())),
            }),
            viewport,
            &mut painter,
        );
        assert_eq!(
            paints.get(),
            2,
            "a widened ambient clip must force a repaint, not reuse a layer \
             that was never painted past the old, narrower clip"
        );
        assert_eq!(
            painter.pixmap.pixel(15, 5).unwrap().red(),
            255,
            "now-visible content must render even though it was never painted \
             the first time the layer was cached under a narrower clip"
        );
    }
}
