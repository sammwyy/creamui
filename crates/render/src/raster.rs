//! CPU rasterization of a [`DisplayList`] with `tiny-skia`.
//!
//! Only damaged regions are cleared and replayed. Primitives entirely inside
//! their clip are drawn straight into the frame; partially clipped ones are
//! drawn into a scratch buffer covering just their visible pixels, so no
//! frame-sized clip mask is ever allocated.

use crate::display_list::{
    Bounds, Clip, Damage, DisplayList, FrameDiff, ImagePrimitive, Line, Primitive, Quad,
    ScrollBlit, TextRun,
};
use creamui_theme::Color;
use std::collections::HashMap;
use tiny_skia::{
    FillRule, FilterQuality, GradientStop, LineCap, LinearGradient, Mask, Paint, PathBuilder,
    Pixmap, PixmapMut, PixmapPaint, PixmapRef, Point, SpreadMode, Stroke, Transform,
};

const TINT_CACHE_FRAMES: u64 = 300;

pub struct Rasterizer {
    pixmap: Pixmap,
    scratch: Vec<u8>,
    images: TintCache,
}

#[derive(Default)]
struct TintCache {
    tinted: HashMap<(u64, Color), (Pixmap, u64)>,
    frame: u64,
}

impl Rasterizer {
    pub fn new(width: u32, height: u32) -> Self {
        Rasterizer {
            pixmap: Pixmap::new(width.max(1), height.max(1)).expect("non-zero pixmap size"),
            scratch: Vec::new(),
            images: TintCache::default(),
        }
    }

    pub fn pixmap(&self) -> &Pixmap {
        &self.pixmap
    }

    /// Brings the frame up to date with `list`, repainting only `damage`.
    /// Returns the regions actually repainted, which widen to the whole
    /// frame after a resize.
    pub fn render(&mut self, list: &DisplayList, damage: &Damage) -> Damage {
        #[cfg(feature = "perf-metrics")]
        let _span = tracing::info_span!("cpu_raster").entered();
        let resized = self.pixmap.width() != list.width || self.pixmap.height() != list.height;
        if resized {
            self.pixmap = Pixmap::new(list.width, list.height).expect("non-zero pixmap size");
        }
        let damage = if resized { &Damage::Full } else { damage };
        let viewport = list.viewport();
        let spaces = list.spaces();
        for region in damage.regions(viewport) {
            self.fill_clear(region, list.clear);
            #[cfg(feature = "perf-metrics")]
            creamui_core::metrics::record(|m| {
                m.cpu_pixels_rasterized += (region.width() * region.height()) as u64
            });
            for item in &list.items {
                if item.visible_bounds(&spaces).intersect(region).is_empty() {
                    continue;
                }
                let clip = item.screen_clip(&spaces);
                let clip = Clip {
                    bounds: clip.bounds.intersect(region),
                    rounded: clip.rounded,
                };
                self.draw(&item.screen_primitive(&spaces), clip);
            }
        }
        self.images.frame += 1;
        let horizon = self.images.frame.saturating_sub(TINT_CACHE_FRAMES);
        self.images.tinted.retain(|_, (_, used)| *used >= horizon);
        damage.clone()
    }

    /// Brings the frame up to date with `list` by shifting and repainting
    /// what `diff` says, and returns every region that changed.
    pub fn apply(&mut self, list: &DisplayList, diff: &FrameDiff) -> Damage {
        let resized = self.pixmap.width() != list.width || self.pixmap.height() != list.height;
        if resized || diff.repaint == Damage::Full {
            return self.render(list, &Damage::Full);
        }
        if let Some(blit) = &diff.scroll {
            self.scroll(blit);
        }
        self.render(list, &diff.repaint);
        diff.changed(list.viewport())
    }

    /// Moves the pixels inside `blit.area` by `(blit.dx, blit.dy)`, leaving
    /// the uncovered part of the area as it was.
    pub fn scroll(&mut self, blit: &ScrollBlit) {
        let (x0, y0, x1, y1) = pixel_span(blit.area.intersect(Bounds::new(
            0.0,
            0.0,
            self.pixmap.width() as f32,
            self.pixmap.height() as f32,
        )));
        let (dx, dy) = (blit.dx as isize, blit.dy as isize);
        let src_x0 = (x0 as isize).max(x0 as isize - dx) as usize;
        let src_x1 = (x1 as isize).min(x1 as isize - dx) as usize;
        if src_x1 <= src_x0 {
            return;
        }
        let width = self.pixmap.width() as usize;
        let row_bytes = (src_x1 - src_x0) * 4;
        let data = self.pixmap.data_mut();
        let mut copy_row = |dst_y: usize| {
            let src_y = dst_y as isize - dy;
            if src_y < y0 as isize || src_y >= y1 as isize {
                return;
            }
            let src = (src_y as usize * width + src_x0) * 4;
            let dst = (dst_y * width) as isize + src_x0 as isize + dx;
            data.copy_within(src..src + row_bytes, dst as usize * 4);
        };
        if dy > 0 {
            (y0..y1).rev().for_each(&mut copy_row);
        } else {
            (y0..y1).for_each(&mut copy_row);
        }
    }

    fn fill_clear(&mut self, region: Bounds, color: Color) {
        let pixel = premultiply(color);
        let width = self.pixmap.width() as usize;
        let (x0, y0, x1, y1) = pixel_span(region);
        let data = self.pixmap.data_mut();
        for y in y0..y1 {
            let row = &mut data[(y * width + x0) * 4..(y * width + x1) * 4];
            for px in row.chunks_exact_mut(4) {
                px.copy_from_slice(&pixel);
            }
        }
    }

    fn draw(&mut self, primitive: &Primitive, clip: Clip) {
        if clip.bounds.is_empty() {
            return;
        }
        if let Primitive::Text(run) = primitive {
            draw_text(&mut self.pixmap, run, clip);
            return;
        }
        let bounds = primitive.bounds();
        if clip.contains(bounds) {
            paint(
                primitive,
                &mut self.pixmap.as_mut(),
                Transform::identity(),
                None,
                &mut self.images,
            );
            return;
        }
        if let Primitive::Quad(quad) = primitive {
            if clip.rounded.is_none()
                && quad.gradient.is_none()
                && quad.radius == 0.0
                && quad.border_width == 0.0
            {
                let visible = quad.bounds.intersect(clip.bounds);
                if !visible.is_empty() {
                    fill_rect(
                        &mut self.pixmap.as_mut(),
                        visible,
                        quad.background,
                        Transform::identity(),
                        None,
                    );
                }
                return;
            }
        }
        self.draw_clipped(primitive, clip, bounds);
    }

    /// Draws `primitive` into a scratch buffer around its visible part and
    /// copies that part back. The buffer extends past the copied part by
    /// the largest corner radius involved: `tiny-skia` flattens a curve cut
    /// by the target's border differently from a whole one, so a corner
    /// must lie entirely inside the buffer for a partial repaint to match a
    /// full one.
    fn draw_clipped(&mut self, primitive: &Primitive, clip: Clip, bounds: Bounds) {
        let frame = Bounds::new(
            0.0,
            0.0,
            self.pixmap.width() as f32,
            self.pixmap.height() as f32,
        );
        let area = bounds.intersect(clip.bounds).round_out().intersect(frame);
        if area.is_empty() {
            return;
        }
        let (x0, y0, x1, y1) = pixel_span(area);
        let curve = match primitive {
            Primitive::Quad(quad) => quad.radius,
            Primitive::Line(line) => line.width / 2.0,
            _ => 0.0,
        }
        .max(clip.rounded.map_or(0.0, |rounded| rounded.radius));
        let padded = bounds
            .round_out()
            .intersect(area.inflate(curve.ceil() + 2.0))
            .intersect(frame);
        let (px0, py0, px1, py1) = pixel_span(padded);
        let (w, h) = (px1 - px0, py1 - py0);
        let frame_width = self.pixmap.width() as usize;
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.resize(w * h * 4, 0);
        copy_rows(
            self.pixmap.data_mut(),
            frame_width,
            &mut scratch,
            w,
            (px0, py0),
            (0, 0),
            (w, h),
            true,
        );

        let transform = Transform::from_translate(-(px0 as f32), -(py0 as f32));
        let mask = clip.rounded.map(|rounded| {
            let mut mask = Mask::new(w as u32, h as u32).expect("non-zero mask size");
            if let Some(path) = rounded_rect_path(rounded.bounds, rounded.radius) {
                mask.fill_path(&path, FillRule::Winding, true, transform);
            }
            mask
        });
        {
            let mut target = PixmapMut::from_bytes(&mut scratch, w as u32, h as u32)
                .expect("scratch buffer matches its dimensions");
            paint(
                primitive,
                &mut target,
                transform,
                mask.as_ref(),
                &mut self.images,
            );
        }
        copy_rows(
            self.pixmap.data_mut(),
            frame_width,
            &mut scratch,
            w,
            (x0, y0),
            (x0 - px0, y0 - py0),
            (x1 - x0, y1 - y0),
            false,
        );
        self.scratch = scratch;
    }
}

fn paint(
    primitive: &Primitive,
    target: &mut PixmapMut,
    transform: Transform,
    mask: Option<&Mask>,
    images: &mut TintCache,
) {
    match primitive {
        Primitive::Quad(quad) => draw_quad(target, quad, transform, mask),
        Primitive::Line(line) => draw_line(target, line, transform, mask),
        Primitive::Image(image) => draw_image(target, image, transform, mask, images),
        Primitive::Text(_) => unreachable!("text is blitted directly"),
    }
}

fn draw_image(
    target: &mut PixmapMut,
    image: &ImagePrimitive,
    transform: Transform,
    mask: Option<&Mask>,
    images: &mut TintCache,
) {
    let (width, height) = (image.image.width(), image.image.height());
    let frame = images.frame;
    let pixels;
    let source = match image.tint {
        None => {
            pixels = image.image.pixels();
            PixmapRef::from_bytes(&pixels, width, height)
        }
        Some(tint) => {
            let (pixmap, used) = images
                .tinted
                .entry((image.image.id(), tint))
                .or_insert_with(|| (tinted(image, tint), frame));
            *used = frame;
            Some(pixmap.as_ref())
        }
    };
    let Some(source) = source else {
        return;
    };
    let b = image.bounds;
    let transform = transform.pre_concat(Transform::from_row(
        b.width() / width as f32,
        0.0,
        0.0,
        b.height() / height as f32,
        b.x0,
        b.y0,
    ));
    let paint = PixmapPaint {
        quality: FilterQuality::Bilinear,
        ..Default::default()
    };
    target.draw_pixmap(0, 0, source, &paint, transform, mask);
}

fn tinted(image: &ImagePrimitive, tint: Color) -> Pixmap {
    let mut pixmap =
        Pixmap::new(image.image.width(), image.image.height()).expect("non-zero image size");
    for (dst, src) in pixmap
        .data_mut()
        .chunks_exact_mut(4)
        .zip(image.image.pixels().chunks_exact(4))
    {
        let alpha = (src[3] as u32 * tint.a as u32 + 127) / 255;
        dst[0] = mul(tint.r, alpha);
        dst[1] = mul(tint.g, alpha);
        dst[2] = mul(tint.b, alpha);
        dst[3] = alpha as u8;
    }
    pixmap
}

fn pixel_span(b: Bounds) -> (usize, usize, usize, usize) {
    (
        b.x0.max(0.0) as usize,
        b.y0.max(0.0) as usize,
        b.x1.max(0.0) as usize,
        b.y1.max(0.0) as usize,
    )
}

/// Copies a `size` block between the frame at `frame_at` and the scratch
/// buffer (`scratch_width` pixels wide) at `scratch_at`.
#[allow(clippy::too_many_arguments)]
fn copy_rows(
    frame: &mut [u8],
    frame_width: usize,
    scratch: &mut [u8],
    scratch_width: usize,
    frame_at: (usize, usize),
    scratch_at: (usize, usize),
    size: (usize, usize),
    into_scratch: bool,
) {
    let (w, h) = size;
    for row in 0..h {
        let src = ((frame_at.1 + row) * frame_width + frame_at.0) * 4;
        let dst = ((scratch_at.1 + row) * scratch_width + scratch_at.0) * 4;
        let (frame_row, scratch_row) =
            (&mut frame[src..src + w * 4], &mut scratch[dst..dst + w * 4]);
        if into_scratch {
            scratch_row.copy_from_slice(frame_row);
        } else {
            frame_row.copy_from_slice(scratch_row);
        }
    }
}

fn mul(channel: u8, alpha: u32) -> u8 {
    ((channel as u32 * alpha + 127) / 255) as u8
}

fn premultiply(color: Color) -> [u8; 4] {
    let a = color.a as u32;
    [mul(color.r, a), mul(color.g, a), mul(color.b, a), color.a]
}

fn solid(color: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = true;
    paint
}

fn quad_paint(quad: &Quad) -> Paint<'static> {
    let Some(gradient) = quad.gradient else {
        return solid(quad.background);
    };
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.shader = LinearGradient::new(
        Point::from_xy(gradient.start[0], gradient.start[1]),
        Point::from_xy(gradient.end[0], gradient.end[1]),
        vec![
            GradientStop::new(
                0.0,
                tiny_skia::Color::from_rgba8(
                    gradient.start_color.r,
                    gradient.start_color.g,
                    gradient.start_color.b,
                    gradient.start_color.a,
                ),
            ),
            GradientStop::new(
                1.0,
                tiny_skia::Color::from_rgba8(
                    gradient.end_color.r,
                    gradient.end_color.g,
                    gradient.end_color.b,
                    gradient.end_color.a,
                ),
            ),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or_else(|| solid(quad.background).shader);
    paint
}

fn fill_rect(
    target: &mut PixmapMut,
    b: Bounds,
    color: Color,
    transform: Transform,
    mask: Option<&Mask>,
) {
    if let Some(rect) = tiny_skia::Rect::from_ltrb(b.x0, b.y0, b.x1, b.y1) {
        target.fill_rect(rect, &solid(color), transform, mask);
    }
}

fn rounded_rect_path(b: Bounds, radius: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    push_rounded_rect(&mut pb, b, radius);
    pb.finish()
}

fn push_rounded_rect(pb: &mut PathBuilder, b: Bounds, radius: f32) {
    let (x, y, w, h) = (b.x0, b.y0, b.width(), b.height());
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    if r <= 0.01 {
        if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) {
            pb.push_rect(rect);
        }
        return;
    }
    const K: f32 = 0.552_284_8;
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

fn draw_quad(target: &mut PixmapMut, quad: &Quad, transform: Transform, mask: Option<&Mask>) {
    if quad.background.a > 0 || quad.gradient.is_some() {
        let paint = quad_paint(quad);
        if quad.radius <= 0.01 {
            if let Some(rect) = tiny_skia::Rect::from_ltrb(
                quad.bounds.x0,
                quad.bounds.y0,
                quad.bounds.x1,
                quad.bounds.y1,
            ) {
                target.fill_rect(rect, &paint, transform, mask);
            }
        } else if let Some(path) = rounded_rect_path(quad.bounds, quad.radius) {
            target.fill_path(&path, &paint, FillRule::Winding, transform, mask);
        }
    }
    if quad.border_width > 0.0 && quad.border_color.a > 0 {
        let mut pb = PathBuilder::new();
        push_rounded_rect(&mut pb, quad.bounds, quad.radius);
        push_rounded_rect(
            &mut pb,
            quad.bounds.inflate(-quad.border_width),
            quad.radius - quad.border_width,
        );
        if let Some(path) = pb.finish() {
            target.fill_path(
                &path,
                &solid(quad.border_color),
                FillRule::EvenOdd,
                transform,
                mask,
            );
        }
    }
}

fn draw_line(target: &mut PixmapMut, line: &Line, transform: Transform, mask: Option<&Mask>) {
    let mut pb = PathBuilder::new();
    pb.move_to(line.from[0], line.from[1]);
    pb.line_to(line.to[0], line.to[1]);
    let Some(path) = pb.finish() else {
        return;
    };
    let stroke = Stroke {
        width: line.width,
        line_cap: LineCap::Round,
        ..Default::default()
    };
    target.stroke_path(&path, &solid(line.color), &stroke, transform, mask);
}

fn rounded_coverage(clip: &Clip, px: i32, py: i32) -> u32 {
    let Some(rounded) = clip.rounded else {
        return 255;
    };
    let b = rounded.bounds;
    let (cx, cy) = ((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0);
    let (hx, hy) = (b.width() / 2.0, b.height() / 2.0);
    let r = rounded.radius;
    let qx = (px as f32 + 0.5 - cx).abs() - hx + r;
    let qy = (py as f32 + 0.5 - cy).abs() - hy + r;
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    let distance = outside + qx.max(qy).min(0.0) - r;
    ((0.5 - distance).clamp(0.0, 1.0) * 255.0).round() as u32
}

fn draw_text(pixmap: &mut Pixmap, run: &TextRun, clip: Clip) {
    let (cx0, cy0, cx1, cy1) = (
        clip.bounds.x0 as i32,
        clip.bounds.y0 as i32,
        clip.bounds.x1 as i32,
        clip.bounds.y1 as i32,
    );
    let width = pixmap.width() as i32;
    let data = pixmap.data_mut();
    for glyph in &run.layout.glyphs {
        let bitmap = &glyph.bitmap;
        let (gw, gh) = (bitmap.width as i32, bitmap.height as i32);
        let gx = run.x + glyph.x;
        let gy = run.y + glyph.y;
        let slant = run.shear(0, bitmap.height);
        if gx >= cx1 || gx + gw + slant <= cx0 || gy >= cy1 || gy + gh <= cy0 {
            continue;
        }
        let color = run.glyph_color(glyph.byte_offset);
        let glyph_bounds = Bounds::new(
            gx as f32,
            gy as f32,
            (gx + gw + slant) as f32,
            (gy + gh) as f32,
        );
        let rounded = clip
            .rounded
            .filter(|r| !r.inner().contains(glyph_bounds))
            .map(|_| clip);
        #[cfg(feature = "perf-metrics")]
        creamui_core::metrics::record(|m| m.cpu_pixels_rasterized += (gw * gh) as u64);
        for row in 0..gh {
            let py = gy + row;
            if py < cy0 || py >= cy1 {
                continue;
            }
            let shear = run.shear(row as u32, bitmap.height);
            let coverage_row = &bitmap.coverage[(row * gw) as usize..((row + 1) * gw) as usize];
            for (col, &coverage) in coverage_row.iter().enumerate() {
                if coverage == 0 {
                    continue;
                }
                let px = gx + col as i32 + shear;
                if px < cx0 || px >= cx1 {
                    continue;
                }
                let mut alpha = coverage as u32 * color.a as u32;
                if let Some(clip) = &rounded {
                    alpha = alpha * rounded_coverage(clip, px, py) / 255;
                }
                let alpha = (alpha + 127) / 255;
                if alpha == 0 {
                    continue;
                }
                let inverse = 255 - alpha;
                let idx = ((py * width + px) * 4) as usize;
                let dst = &mut data[idx..idx + 4];
                dst[0] = ((color.r as u32 * alpha + dst[0] as u32 * inverse + 127) / 255) as u8;
                dst[1] = ((color.g as u32 * alpha + dst[1] as u32 * inverse + 127) / 255) as u8;
                dst[2] = ((color.b as u32 * alpha + dst[2] as u32 * inverse + 127) / 255) as u8;
                dst[3] = ((255 * alpha + dst[3] as u32 * inverse + 127) / 255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_list::damage;
    use crate::recorder::SceneRecorder;
    use creamui_core::{Painter, Rect, RgbaImage, TextAlign};
    use creamui_theme::ColorScheme;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn record(paint: impl FnOnce(&mut SceneRecorder)) -> DisplayList {
        let mut recorder = SceneRecorder::new();
        recorder.begin(40, 30, 1.0, Color::rgb(10, 20, 30), ColorScheme::default());
        paint(&mut recorder);
        recorder.finish()
    }

    fn rgba(r: &Rasterizer, x: u32, y: u32) -> [u8; 4] {
        let p = r.pixmap().pixel(x, y).unwrap();
        [p.red(), p.green(), p.blue(), p.alpha()]
    }

    #[test]
    fn full_render_clears_and_fills() {
        let list = record(|p| p.fill_rect(rect(5.0, 5.0, 10.0, 10.0), Color::rgb(255, 0, 0), 0.0));
        let mut r = Rasterizer::new(40, 30);
        r.render(&list, &Damage::Full);
        assert_eq!(rgba(&r, 0, 0), [10, 20, 30, 255]);
        assert_eq!(rgba(&r, 8, 8), [255, 0, 0, 255]);
    }

    #[test]
    fn linear_gradient_interpolates_across_the_quad() {
        let list = record(|p| {
            p.fill_linear_gradient(
                rect(0.0, 0.0, 40.0, 30.0),
                Color::rgb(255, 0, 0),
                Color::rgb(0, 0, 255),
                90.0,
                0.0,
            )
        });
        let mut raster = Rasterizer::new(40, 30);
        raster.render(&list, &Damage::Full);
        let left = rgba(&raster, 2, 15);
        let right = rgba(&raster, 37, 15);
        assert!(left[0] > left[2]);
        assert!(right[2] > right[0]);
    }

    #[test]
    fn partial_render_repaints_only_the_damaged_region() {
        let first = record(|p| {
            p.fill_rect(rect(0.0, 0.0, 10.0, 10.0), Color::rgb(255, 0, 0), 0.0);
            p.fill_rect(rect(20.0, 0.0, 10.0, 10.0), Color::rgb(255, 0, 0), 0.0);
        });
        let second = record(|p| {
            p.fill_rect(rect(0.0, 0.0, 10.0, 10.0), Color::rgb(255, 0, 0), 0.0);
            p.fill_rect(rect(20.0, 0.0, 10.0, 10.0), Color::rgb(0, 255, 0), 0.0);
        });
        let mut r = Rasterizer::new(40, 30);
        r.render(&first, &Damage::Full);
        r.pixmap.data_mut()[0] = 7;
        let d = damage(Some(&first), &second);
        assert!(matches!(d, Damage::Partial(_)));
        r.render(&second, &d);
        assert_eq!(rgba(&r, 25, 5), [0, 255, 0, 255]);
        assert_eq!(r.pixmap().data()[0], 7, "undamaged pixels stay untouched");
    }

    #[test]
    fn partial_render_matches_a_full_render() {
        let a = record(|p| {
            p.fill_rect(rect(0.0, 0.0, 40.0, 30.0), Color::rgb(40, 40, 40), 6.0);
            p.fill_text(
                rect(2.0, 2.0, 36.0, 12.0),
                "Hey",
                Color::rgb(250, 250, 250),
                11.0,
                TextAlign::Start,
            );
            p.stroke_rect(
                rect(4.0, 16.0, 20.0, 10.0),
                Color::rgb(0, 128, 255),
                2.0,
                3.0,
            );
        });
        let b = record(|p| {
            p.fill_rect(rect(0.0, 0.0, 40.0, 30.0), Color::rgb(40, 40, 40), 6.0);
            p.fill_text(
                rect(2.0, 2.0, 36.0, 12.0),
                "Hey",
                Color::rgb(250, 250, 250),
                11.0,
                TextAlign::Start,
            );
            p.stroke_rect(
                rect(4.0, 16.0, 20.0, 10.0),
                Color::rgb(255, 128, 0),
                2.0,
                3.0,
            );
        });
        let mut incremental = Rasterizer::new(40, 30);
        incremental.render(&a, &Damage::Full);
        incremental.render(&b, &damage(Some(&a), &b));
        let mut full = Rasterizer::new(40, 30);
        full.render(&b, &Damage::Full);
        assert_eq!(incremental.pixmap().data(), full.pixmap().data());
    }

    #[test]
    fn clipped_primitives_stay_inside_the_clip() {
        let list = record(|p| {
            p.push_clip(rect(10.0, 10.0, 10.0, 10.0));
            p.fill_rect(rect(0.0, 0.0, 40.0, 30.0), Color::rgb(255, 0, 0), 4.0);
            p.stroke_line(
                creamui_core::Point { x: 0.0, y: 15.0 },
                creamui_core::Point { x: 40.0, y: 15.0 },
                Color::rgb(0, 255, 0),
                2.0,
            );
            p.pop_clip();
        });
        let mut r = Rasterizer::new(40, 30);
        r.render(&list, &Damage::Full);
        assert_eq!(rgba(&r, 9, 15), [10, 20, 30, 255]);
        assert_eq!(rgba(&r, 20, 15), [10, 20, 30, 255]);
        assert_eq!(rgba(&r, 12, 12), [255, 0, 0, 255]);
        assert_eq!(rgba(&r, 12, 15), [0, 255, 0, 255]);
    }

    #[test]
    fn rounded_clips_cut_corners() {
        let list = record(|p| {
            p.push_clip_rounded(rect(0.0, 0.0, 20.0, 20.0), 10.0);
            p.fill_rect(rect(0.0, 0.0, 20.0, 20.0), Color::rgb(255, 0, 0), 0.0);
            p.fill_text(
                rect(0.0, 0.0, 20.0, 20.0),
                "██",
                Color::rgb(0, 0, 255),
                30.0,
                TextAlign::Start,
            );
            p.pop_clip();
        });
        let mut r = Rasterizer::new(40, 30);
        r.render(&list, &Damage::Full);
        assert_eq!(rgba(&r, 0, 0), [10, 20, 30, 255]);
        assert_eq!(rgba(&r, 10, 10)[3], 255);
    }

    #[test]
    fn tinted_images_keep_alpha_and_take_the_tint() {
        let image = RgbaImage::new(1, 1, vec![128, 0, 0, 128]).unwrap();
        let list = record(|p| {
            p.draw_image(
                rect(0.0, 0.0, 40.0, 30.0),
                &image,
                Some(Color::rgb(0, 0, 255)),
            );
        });
        let mut r = Rasterizer::new(40, 30);
        r.render(&list, &Damage::Full);
        let [red, _, blue, alpha] = rgba(&r, 20, 15);
        assert_eq!(alpha, 255);
        assert!(blue > 130 && red < 10);
        assert_eq!(r.images.tinted.len(), 1);
    }

    #[test]
    fn resizing_forces_a_full_repaint() {
        let list = record(|_| {});
        let mut r = Rasterizer::new(10, 10);
        assert_eq!(r.render(&list, &Damage::None), Damage::Full);
        assert_eq!((r.pixmap().width(), r.pixmap().height()), (40, 30));
    }

    struct ScrollScene {
        offset: f32,
        gradient_backdrop: bool,
        radius: f32,
        overlay: bool,
    }

    const SCROLL_SCENE: ScrollScene = ScrollScene {
        offset: 40.0,
        gradient_backdrop: false,
        radius: 0.0,
        overlay: true,
    };

    fn scroll_scene(scene: &ScrollScene) -> DisplayList {
        let mut r = SceneRecorder::new();
        r.begin(
            120,
            100,
            1.0,
            Color::rgb(10, 20, 30),
            ColorScheme::default(),
        );
        if scene.gradient_backdrop {
            r.fill_linear_gradient(
                rect(0.0, 0.0, 120.0, 100.0),
                Color::rgb(200, 0, 0),
                Color::rgb(0, 0, 200),
                180.0,
                0.0,
            );
        } else {
            r.fill_rect(rect(0.0, 0.0, 120.0, 100.0), Color::rgb(240, 240, 240), 0.0);
        }
        r.push_scroll_layer(
            rect(10.0, 10.0, 90.0, 80.0),
            scene.radius,
            creamui_core::Point {
                x: 0.0,
                y: scene.offset,
            },
        );
        for i in 0..20 {
            let y = 10.0 + i as f32 * 12.0 - scene.offset;
            r.push_clip_rounded(rect(12.0, y, 84.0, 10.0), 3.0);
            r.fill_rect(
                rect(12.0, y, 84.0, 10.0),
                Color::rgb((i * 12) as u8, 90, 160),
                0.0,
            );
            r.fill_text(
                rect(14.0, y, 60.0, 10.0),
                &format!("row {i}"),
                Color::rgb(255, 255, 255),
                8.0,
                TextAlign::Start,
            );
            r.pop_clip();
        }
        r.pop_scroll_layer();
        if scene.overlay {
            r.fill_rect(
                rect(94.0, 10.0 + scene.offset / 4.0, 4.0, 20.0),
                Color::rgba(0, 0, 0, 120),
                2.0,
            );
        }
        r.finish()
    }

    fn assert_incremental_matches_full(before: &DisplayList, after: &DisplayList) -> FrameDiff {
        let mut incremental = Rasterizer::new(before.width, before.height);
        incremental.render(before, &Damage::Full);
        let diff = crate::display_list::diff(Some(before), after);
        incremental.apply(after, &diff);
        let mut full = Rasterizer::new(after.width, after.height);
        full.render(after, &Damage::Full);
        assert!(
            incremental.pixmap().data() == full.pixmap().data(),
            "scrolled frame differs from a full render"
        );
        diff
    }

    #[test]
    fn scrolling_over_a_solid_backdrop_shifts_pixels_and_repaints_the_rest() {
        let before = scroll_scene(&SCROLL_SCENE);
        let after = scroll_scene(&ScrollScene {
            offset: 52.0,
            ..SCROLL_SCENE
        });
        let diff = assert_incremental_matches_full(&before, &after);
        assert_eq!(
            diff.scroll,
            Some(ScrollBlit {
                area: Bounds::new(10.0, 10.0, 100.0, 90.0),
                dx: 0,
                dy: -12,
            })
        );
        assert!(diff.repaint.area(after.viewport()) < 90.0 * 80.0 / 2.0);
    }

    #[test]
    fn scrolling_back_up_matches_a_full_render() {
        let before = scroll_scene(&SCROLL_SCENE);
        let after = scroll_scene(&ScrollScene {
            offset: 17.0,
            ..SCROLL_SCENE
        });
        assert!(assert_incremental_matches_full(&before, &after)
            .scroll
            .is_some());
    }

    #[test]
    fn rounded_viewports_scroll_exactly() {
        let before = scroll_scene(&ScrollScene {
            radius: 8.0,
            ..SCROLL_SCENE
        });
        let after = scroll_scene(&ScrollScene {
            offset: 64.0,
            radius: 8.0,
            ..SCROLL_SCENE
        });
        assert!(assert_incremental_matches_full(&before, &after)
            .scroll
            .is_some());
    }

    #[test]
    fn a_patterned_backdrop_or_fractional_offset_repaints_instead() {
        let gradient = |offset| {
            scroll_scene(&ScrollScene {
                offset,
                gradient_backdrop: true,
                ..SCROLL_SCENE
            })
        };
        let diff = assert_incremental_matches_full(&gradient(40.0), &gradient(52.0));
        assert!(diff.scroll.is_none());

        let fractional = scroll_scene(&ScrollScene {
            offset: 45.5,
            ..SCROLL_SCENE
        });
        let diff = assert_incremental_matches_full(&scroll_scene(&SCROLL_SCENE), &fractional);
        assert!(diff.scroll.is_none());
    }

    #[test]
    fn scroll_layer_items_do_not_depend_on_the_offset() {
        let before = scroll_scene(&ScrollScene {
            overlay: false,
            ..SCROLL_SCENE
        });
        let after = scroll_scene(&ScrollScene {
            offset: 41.0,
            overlay: false,
            ..SCROLL_SCENE
        });
        let row = |list: &DisplayList| {
            list.items
                .iter()
                .find(|item| item.layer == 1 && matches!(item.primitive, Primitive::Quad(_)))
                .cloned()
                .unwrap()
        };
        assert!(row(&before) == row(&after));
        assert_eq!(after.layers[0].offset, [0.0, -41.0]);
    }
}
