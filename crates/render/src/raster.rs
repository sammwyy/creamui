//! CPU rasterization of a [`DisplayList`] with `tiny-skia`.
//!
//! Only damaged regions are cleared and replayed. Primitives entirely inside
//! their clip are drawn straight into the frame; partially clipped ones are
//! drawn into a scratch buffer covering just their visible pixels, so no
//! frame-sized clip mask is ever allocated.

use crate::display_list::{
    Bounds, Clip, Damage, DisplayList, FrameDiff, ImagePrimitive, Line, Primitive, Quad,
    QuadGradient, RoundedClip, ScrollBlit, TextRun,
};
use creamui_core::ResolvedRadialColorStop;
use creamui_theme::Color;
use std::collections::HashMap;
use tiny_skia::{
    FillRule, FilterQuality, GradientStop, LineCap, LinearGradient, Mask, Paint, PathBuilder,
    Pixmap, PixmapMut, PixmapPaint, PixmapRef, Point, SpreadMode, Stroke, Transform,
};

const TINT_CACHE_FRAMES: u64 = 300;
const SCALED_TINT_CACHE_BYTES: usize = 8 * 1024 * 1024;

pub struct Rasterizer {
    pixmap: Pixmap,
    scratch: Vec<u8>,
    images: TintCache,
    solid_fast_path: bool,
}

#[derive(Default)]
struct TintCache {
    tinted: HashMap<(u64, Color), (Pixmap, u64)>,
    scaled: HashMap<ScaledTintKey, (Pixmap, u64)>,
    scaled_bytes: usize,
    frame: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ScaledTintKey {
    image: u64,
    tint: Color,
    transform: [u32; 6],
    crop: [u32; 4],
    matrix: [u32; 12],
}

impl TintCache {
    fn store_scaled(&mut self, key: ScaledTintKey, pixmap: Pixmap) {
        let bytes = pixmap.data().len();
        if bytes > SCALED_TINT_CACHE_BYTES {
            return;
        }
        while self.scaled_bytes + bytes > SCALED_TINT_CACHE_BYTES {
            let key = *self
                .scaled
                .iter()
                .min_by_key(|(_, (_, used))| used)
                .unwrap()
                .0;
            let (old, _) = self.scaled.remove(&key).unwrap();
            self.scaled_bytes -= old.data().len();
        }
        self.scaled_bytes += bytes;
        self.scaled.insert(key, (pixmap, self.frame));
    }
}

impl Rasterizer {
    pub fn new(width: u32, height: u32) -> Self {
        Rasterizer {
            pixmap: Pixmap::new(width.max(1), height.max(1)).expect("non-zero pixmap size"),
            scratch: Vec::new(),
            images: TintCache::default(),
            solid_fast_path: true,
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
            let frame_bytes = (list.width as usize)
                .saturating_mul(list.height as usize)
                .saturating_mul(4);
            if self.scratch.capacity() > frame_bytes.saturating_mul(4) {
                self.scratch = Vec::new();
            }
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
        self.images.scaled.retain(|_, (pixmap, used)| {
            if *used >= horizon {
                return true;
            }
            self.images.scaled_bytes -= pixmap.data().len();
            false
        });
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
        if let Primitive::Quad(quad) = primitive {
            if self.draw_solid_quad(quad, &clip) {
                return;
            }
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

    /// Draws a large solid quad's fully covered fill and border pixels
    /// directly and leaves only anti-aliased edges and corners to
    /// `tiny-skia`, whose generic pipeline is many times slower per pixel.
    /// Blends exactly as `tiny-skia` does, so the result is identical.
    /// Returns `false`, having drawn nothing, when neither part qualifies.
    fn draw_solid_quad(&mut self, quad: &Quad, clip: &Clip) -> bool {
        if !self.solid_fast_path {
            return false;
        }
        let fill = fill_plan(quad).map(|plan| plan.clipped(clip));
        let border = border_plan(quad).map(|plan| plan.clipped(clip));
        if fill.is_none() && border.is_none() {
            return false;
        }
        let fill_only = Quad {
            border_width: 0.0,
            ..quad.clone()
        };
        match fill {
            Some(plan) => self.draw_plan(&plan, Primitive::Quad(fill_only), clip),
            None if quad.background.a > 0 || quad.gradient.is_some() => {
                self.draw(&Primitive::Quad(fill_only), clip.clone())
            }
            None => {}
        }
        let border_only = Quad {
            background: Color::rgba(0, 0, 0, 0),
            gradient: None,
            ..quad.clone()
        };
        match border {
            Some(plan) => self.draw_plan(&plan, Primitive::Quad(border_only), clip),
            None if quad.border_width > 0.0 && quad.border_color.a > 0 => {
                self.draw(&Primitive::Quad(border_only), clip.clone())
            }
            None => {}
        }
        true
    }

    fn draw_plan(&mut self, plan: &SolidPlan, primitive: Primitive, clip: &Clip) {
        let bounds = primitive.bounds();
        let color = premultiply_like_tiny_skia(plan.color);
        for part in &plan.solid {
            let part = part.intersect(clip.bounds);
            if !part.is_empty() {
                blend_solid(&mut self.pixmap, part, color);
            }
        }
        for rim in &plan.rims {
            let rim = rim.intersect(clip.bounds);
            if !rim.is_empty() {
                let rim_clip = Clip {
                    bounds: rim,
                    rounded: clip.rounded.clone(),
                };
                self.draw_clipped(&primitive, rim_clip, bounds);
            }
        }
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
        .max(
            clip.rounded_clips()
                .map(|rounded| rounded.radius)
                .fold(0.0, f32::max),
        );
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
        let mut mask: Option<Mask> = None;
        for rounded in clip.rounded_clips() {
            if rounded.inner().contains(area) {
                continue;
            }
            let Some(path) = rounded_rect_path(rounded.bounds, rounded.radius) else {
                continue;
            };
            if let Some(existing) = &mut mask {
                existing.intersect_path(&path, FillRule::Winding, true, transform);
            } else {
                let mut first = Mask::new(w as u32, h as u32).expect("non-zero mask size");
                first.fill_path(&path, FillRule::Winding, true, transform);
                mask = Some(first);
            }
        }
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
        Primitive::NativeImage(image) => draw_quad(
            target,
            &Quad {
                bounds: image.bounds,
                background: Color::rgb(32, 32, 32),
                gradient: None,
                radius: 0.0,
                border_width: 0.0,
                border_color: Color::rgba(0, 0, 0, 0),
            },
            transform,
            mask,
        ),
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
    let b = image.bounds;
    let tint_after_scaling = image.tint.filter(|_| {
        let source_area = width as u64 * height as u64;
        source_area >= 256 * 256
            && source_area as f64 >= (b.width() as f64 * b.height() as f64) * 16.0
    });
    let frame = images.frame;
    let pixels;
    let source = match image.tint.filter(|_| tint_after_scaling.is_none()) {
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
    let mapping = match image.effects.quarter_turns {
        1 => Transform::from_row(
            0.0,
            b.height() / width as f32,
            -b.width() / height as f32,
            0.0,
            b.x1,
            b.y0,
        ),
        2 => Transform::from_row(
            -b.width() / width as f32,
            0.0,
            0.0,
            -b.height() / height as f32,
            b.x1,
            b.y1,
        ),
        3 => Transform::from_row(
            0.0,
            -b.height() / width as f32,
            b.width() / height as f32,
            0.0,
            b.x0,
            b.y1,
        ),
        _ => Transform::from_row(
            b.width() / width as f32,
            0.0,
            0.0,
            b.height() / height as f32,
            b.x0,
            b.y0,
        ),
    };
    let transform = transform.pre_concat(mapping);
    let paint = PixmapPaint {
        quality: FilterQuality::Bilinear,
        opacity: image.opacity,
        ..Default::default()
    };
    if image.effects.has_color_transform() || tint_after_scaling.is_some() {
        let mut corners = [
            Point::from_xy(0.0, 0.0),
            Point::from_xy(width as f32, 0.0),
            Point::from_xy(width as f32, height as f32),
            Point::from_xy(0.0, height as f32),
        ];
        transform.map_points(&mut corners);
        let x0 = (corners
            .iter()
            .map(|point| point.x)
            .fold(f32::INFINITY, f32::min)
            .floor()
            - 1.0)
            .max(0.0) as u32;
        let y0 = (corners
            .iter()
            .map(|point| point.y)
            .fold(f32::INFINITY, f32::min)
            .floor()
            - 1.0)
            .max(0.0) as u32;
        let x1 = (corners
            .iter()
            .map(|point| point.x)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            + 1.0)
            .clamp(0.0, target.width() as f32) as u32;
        let y1 = (corners
            .iter()
            .map(|point| point.y)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            + 1.0)
            .clamp(0.0, target.height() as f32) as u32;
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let scaled_key = tint_after_scaling.map(|tint| ScaledTintKey {
            image: image.image.id(),
            tint,
            transform: [
                transform.sx.to_bits(),
                transform.kx.to_bits(),
                transform.ky.to_bits(),
                transform.sy.to_bits(),
                transform.tx.to_bits(),
                transform.ty.to_bits(),
            ],
            crop: [x0, y0, x1, y1],
            matrix: std::array::from_fn(|i| image.effects.color_matrix[i / 4][i % 4].to_bits()),
        });
        if let Some((cached, used)) = scaled_key.and_then(|key| images.scaled.get_mut(&key)) {
            *used = images.frame;
            target.draw_pixmap(
                x0 as i32,
                y0 as i32,
                cached.as_ref(),
                &paint,
                Transform::identity(),
                mask,
            );
            return;
        }
        let mut filtered = Pixmap::new(x1 - x0, y1 - y0).expect("nonempty visible image");
        filtered.draw_pixmap(
            0,
            0,
            source,
            &PixmapPaint {
                quality: FilterQuality::Bilinear,
                ..Default::default()
            },
            transform.post_translate(-(x0 as f32), -(y0 as f32)),
            None,
        );
        for pixel in filtered.data_mut().chunks_exact_mut(4) {
            if let Some(tint) = tint_after_scaling {
                let alpha = (pixel[3] as u32 * tint.a as u32 + 127) / 255;
                pixel.copy_from_slice(&[
                    mul(tint.r, alpha),
                    mul(tint.g, alpha),
                    mul(tint.b, alpha),
                    alpha as u8,
                ]);
            }
            if image.effects.has_color_transform() {
                image.effects.transform_pixel(pixel);
            }
        }
        target.draw_pixmap(
            x0 as i32,
            y0 as i32,
            filtered.as_ref(),
            &paint,
            Transform::identity(),
            mask,
        );
        if let Some(key) = scaled_key {
            images.store_scaled(key, filtered);
        }
    } else {
        target.draw_pixmap(0, 0, source, &paint, transform, mask);
    }
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

/// Disjoint, pixel-aligned parts of a quad's fill or border: `solid` ones
/// the shape covers completely in `color`, and `rims` covering the rest of
/// it, which still need anti-aliasing.
struct SolidPlan {
    color: Color,
    solid: Vec<Bounds>,
    rims: Vec<Bounds>,
}

impl SolidPlan {
    const MIN_SOLID_PIXELS: f32 = 64.0 * 64.0;

    /// Restricts the solid parts to `clip`, moving whatever its rounded
    /// corners could cut into the rims.
    fn clipped(mut self, clip: &Clip) -> Self {
        let mut solid: Vec<Bounds> = self
            .solid
            .iter()
            .map(|part| part.intersect(clip.bounds))
            .filter(|part| !part.is_empty())
            .collect();
        for rounded in clip.rounded_clips() {
            let mut inside = Vec::new();
            for part in solid {
                if rounded.inner().contains(part) {
                    inside.push(part);
                    continue;
                }
                let b = rounded.bounds;
                let r = rounded.radius.min(b.width() / 2.0).min(b.height() / 2.0);
                let (top, left) = ((b.y0 + r).ceil(), (b.x0 + r).ceil());
                let (bottom, right) = ((b.y1 - r).floor().max(top), (b.x1 - r).floor().max(left));
                let (edge0, edge1) = (b.x0.ceil(), b.x1.floor().max(b.x0.ceil()));
                let (above, below) = (b.y0.ceil(), b.y1.floor());
                for (y0, y1, x0, x1) in [
                    (f32::MIN, above, f32::MAX, f32::MAX),
                    (above, top, left, right),
                    (top, bottom, edge0, edge1),
                    (bottom, below, left, right),
                    (below, f32::MAX, f32::MAX, f32::MAX),
                ] {
                    let band = part.intersect(Bounds::new(f32::MIN, y0, f32::MAX, y1));
                    for (piece, solid) in [
                        (Bounds::new(f32::MIN, y0, x0, y1), false),
                        (Bounds::new(x0, y0, x1, y1), true),
                        (Bounds::new(x1, y0, f32::MAX, y1), false),
                    ] {
                        let piece = band.intersect(piece);
                        if piece.is_empty() {
                            continue;
                        }
                        if solid {
                            inside.push(piece);
                        } else {
                            self.rims.push(piece);
                        }
                    }
                }
            }
            solid = inside;
        }
        self.solid = solid;
        self
    }
}

/// The pixel grid around a quad: its rounded-out bounds, and the pixel
/// lines `corner` in from each edge, past which corners can't reach.
struct QuadGrid {
    outer: Bounds,
    inner: Bounds,
}

impl QuadGrid {
    fn new(b: Bounds, corner: f32) -> Option<Self> {
        let inner = Bounds::new(
            (b.x0 + corner).ceil(),
            (b.y0 + corner).ceil(),
            (b.x1 - corner).floor(),
            (b.y1 - corner).floor(),
        );
        (inner.x1 > inner.x0 && inner.y1 > inner.y0).then(|| QuadGrid {
            outer: b.round_out(),
            inner,
        })
    }

    fn corners(&self) -> [Bounds; 4] {
        let (o, i) = (self.outer, self.inner);
        [
            Bounds::new(o.x0, o.y0, i.x0, i.y0),
            Bounds::new(i.x1, o.y0, o.x1, i.y0),
            Bounds::new(o.x0, i.y1, i.x0, o.y1),
            Bounds::new(i.x1, i.y1, o.x1, o.y1),
        ]
    }
}

/// Splits `[start, end)` into fully covered pixels and the partly covered
/// pixel at each fractional end.
fn pixel_spans(start: f32, end: f32) -> ((f32, f32), [(f32, f32); 2]) {
    (
        (start.ceil(), end.floor()),
        [(start.floor(), start.ceil()), (end.floor(), end.ceil())],
    )
}

fn effective_radius(quad: &Quad) -> f32 {
    let b = quad.bounds;
    let r = quad.radius.min(b.width() / 2.0).min(b.height() / 2.0);
    if r <= 0.01 {
        0.0
    } else {
        r
    }
}

fn fill_plan(quad: &Quad) -> Option<SolidPlan> {
    if quad.gradient.is_some() || quad.background.a == 0 {
        return None;
    }
    let b = quad.bounds;
    let grid = QuadGrid::new(b, effective_radius(quad))?;
    let (o, i) = (grid.outer, grid.inner);
    let ((xa, xb), [left, right]) = pixel_spans(b.x0, b.x1);
    let ((ya, yb), [top, bottom]) = pixel_spans(b.y0, b.y1);
    if (xb - xa) * (i.y1 - i.y0) < SolidPlan::MIN_SOLID_PIXELS {
        return None;
    }
    let mut rims = grid.corners().to_vec();
    rims.extend([
        Bounds::new(i.x0, top.0, i.x1, top.1),
        Bounds::new(i.x0, bottom.0, i.x1, bottom.1),
        Bounds::new(left.0, i.y0, left.1, i.y1),
        Bounds::new(right.0, i.y0, right.1, i.y1),
    ]);
    let _ = o;
    Some(SolidPlan {
        color: quad.background,
        solid: vec![
            Bounds::new(xa, i.y0, xb, i.y1),
            Bounds::new(i.x0, ya, i.x1, i.y0),
            Bounds::new(i.x0, i.y1, i.x1, yb),
        ],
        rims,
    })
}

fn border_plan(quad: &Quad) -> Option<SolidPlan> {
    let (b, bw) = (quad.bounds, quad.border_width);
    if bw < 1.0 || quad.border_color.a == 0 {
        return None;
    }
    let grid = QuadGrid::new(b, effective_radius(quad).max(bw))?;
    let i = grid.inner;
    let (mut solid, mut rims) = (Vec::new(), grid.corners().to_vec());
    for (start, end, horizontal) in [
        (b.y0, b.y0 + bw, true),
        (b.y1 - bw, b.y1, true),
        (b.x0, b.x0 + bw, false),
        (b.x1 - bw, b.x1, false),
    ] {
        let (full, partial) = pixel_spans(start, end);
        let band = |(a, z): (f32, f32)| {
            if horizontal {
                Bounds::new(i.x0, a, i.x1, z)
            } else {
                Bounds::new(a, i.y0, z, i.y1)
            }
        };
        solid.push(band(full));
        rims.extend(partial.map(band));
    }
    let area: f32 = solid
        .iter()
        .map(|part| part.width().max(0.0) * part.height().max(0.0))
        .sum();
    (area >= SolidPlan::MIN_SOLID_PIXELS / 4.0).then_some(SolidPlan {
        color: quad.border_color,
        solid,
        rims,
    })
}

/// `color` premultiplied and quantized the way `tiny-skia`'s uniform-color
/// stage does it.
fn premultiply_like_tiny_skia(color: Color) -> [u8; 4] {
    let c = tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a).premultiply();
    [c.red(), c.green(), c.blue(), c.alpha()].map(|v| (v * 255.0 + 0.5) as u8)
}

/// Source-over blends the premultiplied `color` onto the pixel-aligned
/// `area`, with `tiny-skia`'s lowp arithmetic.
fn blend_solid(pixmap: &mut Pixmap, area: Bounds, color: [u8; 4]) {
    let width = pixmap.width() as usize;
    let (x0, y0, x1, y1) = pixel_span(area);
    let (x1, y1) = (x1.min(width), y1.min(pixmap.height() as usize));
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let data = pixmap.data_mut();
    for y in y0..y1 {
        blend_row(&mut data[(y * width + x0) * 4..(y * width + x1) * 4], color);
    }
}

/// Source-over blends the premultiplied `color` onto a row of pixels.
fn blend_row(row: &mut [u8], color: [u8; 4]) {
    let pattern: [u8; 64] = std::array::from_fn(|i| color[i % 4]);
    let inv = 255 - color[3] as u16;
    let mut chunks = row.chunks_exact_mut(pattern.len());
    if inv == 0 {
        for chunk in &mut chunks {
            chunk.copy_from_slice(&pattern);
        }
        let rest = chunks.into_remainder();
        let len = rest.len();
        rest.copy_from_slice(&pattern[..len]);
        return;
    }
    for chunk in &mut chunks {
        for (dst, src) in chunk.iter_mut().zip(pattern) {
            *dst = src + ((*dst as u16 * inv + 255) >> 8) as u8;
        }
    }
    for (dst, src) in chunks.into_remainder().iter_mut().zip(pattern) {
        *dst = src + ((*dst as u16 * inv + 255) >> 8) as u8;
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
    let Some(gradient) = quad.gradient.as_ref() else {
        return solid(quad.background);
    };
    let mut paint = Paint::default();
    paint.anti_alias = true;
    let stops = vec![
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
    ];
    let start = Point::from_xy(gradient.start[0], gradient.start[1]);
    let end = Point::from_xy(gradient.end[0], gradient.end[1]);
    paint.shader = if gradient.radial {
        let radii = [
            gradient.end[0] - gradient.start[0],
            gradient.end[1] - gradient.start[1],
        ];
        tiny_skia::RadialGradient::new(
            Point::from_xy(0.0, 0.0),
            Point::from_xy(0.0, 0.0),
            1.0,
            stops,
            if gradient.repeating {
                SpreadMode::Repeat
            } else {
                SpreadMode::Pad
            },
            Transform::from_row(
                radii[0],
                0.0,
                0.0,
                radii[1],
                gradient.start[0],
                gradient.start[1],
            ),
        )
    } else {
        LinearGradient::new(start, end, stops, SpreadMode::Pad, Transform::identity())
    }
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
    let pixel_gradient = quad.gradient.as_ref().filter(|gradient| {
        gradient.stops.as_ref().is_some_and(|stops| {
            stops.len() != 2
                || stops[0].offset != 0.0
                || stops[1].offset != 1.0
                || stops[0].color.a != stops[1].color.a
        }) || gradient.start_color.a != gradient.end_color.a
    });
    if let Some(gradient) = pixel_gradient {
        draw_gradient_pixels(target, quad, gradient, transform, mask);
    } else if quad.background.a > 0 || quad.gradient.is_some() {
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

fn sample_gradient_stops(
    stops: &[ResolvedRadialColorStop],
    colors: &[[f32; 4]],
    progress: f32,
    repeating: bool,
) -> [f32; 4] {
    let first = stops[0].offset;
    let last = stops[stops.len() - 1].offset;
    let t = if repeating {
        let period = last - first;
        if period <= 1e-6 {
            return colors[colors.len() - 1];
        }
        first + (progress - first).rem_euclid(period)
    } else {
        progress
    };
    if t < first {
        return colors[0];
    }
    if t >= last {
        return colors[colors.len() - 1];
    }
    let upper = stops.partition_point(|stop| stop.offset <= t);
    let lower = upper - 1;
    let fraction =
        ((t - stops[lower].offset) / (stops[upper].offset - stops[lower].offset)).clamp(0.0, 1.0);
    std::array::from_fn(|channel| {
        colors[lower][channel] + (colors[upper][channel] - colors[lower][channel]) * fraction
    })
}

fn draw_gradient_pixels(
    target: &mut PixmapMut,
    quad: &Quad,
    gradient: &QuadGradient,
    transform: Transform,
    mask: Option<&Mask>,
) {
    let bounds = quad.bounds.translate([transform.tx, transform.ty]);
    let shape = RoundedClip {
        bounds,
        radius: quad.radius,
    };
    let width = target.width() as usize;
    let visible = bounds.round_out().intersect(Bounds::new(
        0.0,
        0.0,
        target.width() as f32,
        target.height() as f32,
    ));
    let (x0, y0, x1, y1) = pixel_span(visible);
    let start_point = [
        gradient.start[0] + transform.tx,
        gradient.start[1] + transform.ty,
    ];
    let direction = [
        gradient.end[0] - gradient.start[0],
        gradient.end[1] - gradient.start[1],
    ];
    let length_squared = (direction[0] * direction[0] + direction[1] * direction[1]).max(1e-6);
    let along = [direction[0] / length_squared, direction[1] / length_squared];
    let premultiply = |color: Color| {
        let alpha = color.a as f32 / 255.0;
        [
            color.r as f32 * alpha,
            color.g as f32 * alpha,
            color.b as f32 * alpha,
            color.a as f32,
        ]
    };
    let simple_stops = [
        ResolvedRadialColorStop {
            offset: 0.0,
            color: gradient.start_color,
        },
        ResolvedRadialColorStop {
            offset: 1.0,
            color: gradient.end_color,
        },
    ];
    let stops = gradient.stops.as_deref().unwrap_or(&simple_stops);
    let colors = stops
        .iter()
        .map(|stop| premultiply(stop.color))
        .collect::<Vec<_>>();
    let two_stops = (stops.len() == 2
        && stops[0].offset == 0.0
        && stops[1].offset == 1.0
        && !gradient.repeating)
        .then(|| {
            let (from, to) = (colors[0], colors[1]);
            (from, std::array::from_fn::<f32, 4, _>(|c| to[c] - from[c]))
        });
    let pixels = target.data_mut();
    let (cx, cy) = ((bounds.x0 + bounds.x1) / 2.0, (bounds.y0 + bounds.y1) / 2.0);
    let (hx, hy, r) = (bounds.width() / 2.0, bounds.height() / 2.0, quad.radius);
    let shade = |pixel: &mut [u8], x: usize, y: usize, coverage: f32| {
        let offset = [
            x as f32 + 0.5 - start_point[0],
            y as f32 + 0.5 - start_point[1],
        ];
        let t = if gradient.radial {
            (offset[0] / direction[0].abs().max(1e-6))
                .hypot(offset[1] / direction[1].abs().max(1e-6))
        } else {
            offset[0] * along[0] + offset[1] * along[1]
        };
        let sampled = match two_stops {
            Some((from, delta)) => {
                let t = t.clamp(0.0, 1.0);
                std::array::from_fn(|channel| from[channel] + delta[channel] * t)
            }
            None => sample_gradient_stops(stops, &colors, t, gradient.repeating),
        };
        let alpha = sampled[3] * coverage * (1.0 / 255.0);
        for channel in 0..4 {
            let source = sampled[channel] * coverage;
            pixel[channel] = (source + pixel[channel] as f32 * (1.0 - alpha) + 0.5) as u8;
        }
    };
    let uniform_span = match two_stops {
        Some(stops) if mask.is_none() && !gradient.radial => Some(stops),
        _ => None,
    };
    let lut = uniform_span.map(|(from, delta)| gradient_lut(from, delta));
    for y in y0..y1 {
        let qy = (y as i32 as f32 + 0.5 - cy).abs() - hy + r;
        let full = |x: usize| {
            let qx = (x as i32 as f32 + 0.5 - cx).abs() - hx + r;
            qx <= 0.0 && qy <= 0.0 && qx.max(qy) - r <= -0.5
        };
        let (mut span0, mut span1) = (x1, x1);
        if uniform_span.is_some() {
            span0 = (x0..x1).find(|&x| full(x)).unwrap_or(x1);
            span1 = (span0..x1)
                .rev()
                .find(|&x| full(x))
                .map_or(span0, |x| x + 1);
        }
        for x in (x0..span0).chain(span1..x1) {
            let index = y * width + x;
            let mut coverage = if full(x) {
                1.0
            } else {
                rounded_coverage(shape, x as i32, y as i32) as f32 / 255.0
            };
            if let Some(mask) = mask {
                coverage *= mask.data()[index] as f32 / 255.0;
            }
            if coverage != 0.0 {
                shade(&mut pixels[index * 4..index * 4 + 4], x, y, coverage);
            }
        }
        let Some(lut) = lut.as_deref().filter(|_| span1 > span0) else {
            continue;
        };
        let scale = (lut.len() - 1) as f64 * LUT_FRACTION;
        let frame_y = y as f64 - transform.ty as f64 + 0.5 - gradient.start[1] as f64;
        let row_t = (frame_y * along[1] as f64 * scale + LUT_FRACTION / 2.0) as i64;
        let step = (along[0] as f64 * scale) as i64;
        let frame_x0 = 0.5 - gradient.start[0] as f64;
        let origin = row_t + (frame_x0 * along[0] as f64 * scale) as i64;
        let frame_offset = -(transform.tx as i64);
        let last = lut.len() as i64 - 1;
        let row = &mut pixels[(y * width + span0) * 4..(y * width + span1) * 4];
        let index = |x: usize| {
            let t = origin.saturating_add((x as i64 + frame_offset).saturating_mul(step));
            (t >> LUT_FRACTION_BITS).clamp(0, last) as usize
        };
        let mut start = span0;
        while start < span1 {
            let color = index(start);
            let end = (start + 1..span1)
                .find(|&x| index(x) != color)
                .unwrap_or(span1);
            blend_row(&mut row[(start - span0) * 4..(end - span0) * 4], lut[color]);
            start = end;
        }
    }
}

const LUT_FRACTION_BITS: u32 = 16;
const LUT_FRACTION: f64 = (1u64 << LUT_FRACTION_BITS) as f64;

/// Premultiplied colors along a two-stop gradient, at least one per
/// quarter of a color level so neighbouring entries look the same.
fn gradient_lut(from: [f32; 4], delta: [f32; 4]) -> Vec<[u8; 4]> {
    let range = delta.iter().fold(0.0f32, |range, d| range.max(d.abs()));
    let steps = ((range * 4.0).ceil() as usize + 1).clamp(2, 1024);
    (0..steps)
        .map(|step| {
            let t = step as f32 / (steps - 1) as f32;
            let [r, g, b, a] = std::array::from_fn(|c| (from[c] + delta[c] * t + 0.5) as u8);
            [r.min(a), g.min(a), b.min(a), a]
        })
        .collect()
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

fn rounded_coverage(rounded: RoundedClip, px: i32, py: i32) -> u32 {
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
            .rounded_clips()
            .any(|rounded| !rounded.inner().contains(glyph_bounds));
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
                if rounded {
                    for boundary in clip.rounded_clips() {
                        alpha = alpha * rounded_coverage(boundary, px, py) / 255;
                    }
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
    use crate::display_list::{damage, DrawItem};
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

    fn big_quads(clip: Clip) -> DisplayList {
        let quad = |bounds: Bounds, background: Color, radius: f32, border_width: f32| DrawItem {
            primitive: Primitive::Quad(Quad {
                bounds,
                background,
                gradient: None,
                radius,
                border_width,
                border_color: Color::rgba(255, 255, 255, 40),
            }),
            clip: clip.clone(),
            layer: 0,
        };
        DisplayList {
            width: 240,
            height: 180,
            clear: Color::rgba(0, 0, 0, 0),
            items: vec![
                quad(
                    Bounds::new(0.0, 0.0, 240.0, 180.0),
                    Color::rgba(20, 20, 20, 240),
                    0.0,
                    0.0,
                ),
                quad(
                    Bounds::new(12.0, 0.0, 240.0, 180.0),
                    Color::rgba(200, 30, 90, 240),
                    0.0,
                    0.0,
                ),
                quad(
                    Bounds::new(20.3, 10.6, 220.5, 170.2),
                    Color::rgba(30, 30, 30, 200),
                    16.0,
                    1.0,
                ),
                quad(
                    Bounds::new(40.0, 40.0, 180.0, 150.0),
                    Color::rgb(0, 128, 255),
                    9.5,
                    0.0,
                ),
                quad(
                    Bounds::new(60.5, 70.25, 200.75, 160.5),
                    Color::rgba(255, 255, 255, 12),
                    0.0,
                    2.0,
                ),
                quad(
                    Bounds::new(8.0, 6.0, 232.0, 174.0),
                    Color::rgba(0, 0, 0, 0),
                    16.0,
                    1.0,
                ),
                quad(
                    Bounds::new(15.5, 20.25, 230.75, 175.5),
                    Color::rgba(0, 0, 0, 0),
                    12.0,
                    1.5,
                ),
                quad(
                    Bounds::new(30.0, 30.0, 210.0, 160.0),
                    Color::rgba(0, 0, 0, 0),
                    0.0,
                    3.0,
                ),
                quad(
                    Bounds::new(50.0, 50.0, 150.0, 120.0),
                    Color::rgba(0, 0, 0, 0),
                    2.0,
                    6.0,
                ),
            ],
            layers: Vec::new(),
        }
    }

    fn first_difference(
        a: &[u8],
        b: &[u8],
        width: usize,
    ) -> Option<(usize, usize, [u8; 4], [u8; 4])> {
        let i = a.chunks(4).zip(b.chunks(4)).position(|(x, y)| x != y)?;
        Some((
            i % width,
            i / width,
            a[i * 4..i * 4 + 4].try_into().unwrap(),
            b[i * 4..i * 4 + 4].try_into().unwrap(),
        ))
    }

    #[test]
    fn linear_gradient_partial_repaint_matches_full() {
        let gradient = |bounds: Bounds, radius: f32| DrawItem {
            primitive: Primitive::Quad(Quad {
                bounds,
                background: Color::rgba(244, 248, 251, 220),
                gradient: Some(QuadGradient {
                    start: [bounds.x0, bounds.y0],
                    end: [bounds.x1, bounds.y1],
                    start_color: Color::rgba(244, 248, 251, 220),
                    end_color: Color::rgba(31, 143, 249, 160),
                    radial: false,
                    repeating: false,
                    stops: None,
                }),
                radius,
                border_width: 0.0,
                border_color: Color::rgba(0, 0, 0, 0),
            }),
            clip: Clip {
                bounds: Bounds::new(0.0, 0.0, 240.0, 180.0),
                rounded: None,
            },
            layer: 0,
        };
        let list = DisplayList {
            width: 240,
            height: 180,
            clear: Color::rgba(10, 20, 30, 200),
            items: vec![
                gradient(Bounds::new(0.0, 0.0, 240.0, 180.0), 0.0),
                gradient(Bounds::new(20.3, 10.6, 220.5, 170.2), 14.0),
            ],
            layers: Vec::new(),
        };
        let mut full = Rasterizer::new(240, 180);
        full.render(&list, &Damage::Full);
        let mut partial = Rasterizer::new(240, 180);
        partial.render(&list, &Damage::Full);
        partial.render(
            &list,
            &Damage::Partial(vec![
                Bounds::new(0.0, 0.0, 100.0, 64.0),
                Bounds::new(150.0, 90.0, 240.0, 180.0),
            ]),
        );
        assert_eq!(
            first_difference(partial.pixmap().data(), full.pixmap().data(), 240),
            None
        );
    }

    #[test]
    fn large_solid_quads_match_tiny_skia_exactly() {
        let rounded = |radius: f32| {
            let mut clip = Clip {
                bounds: Bounds::new(0.0, 0.0, 240.0, 180.0),
                rounded: None,
            };
            clip.push_rounded(RoundedClip {
                bounds: Bounds::new(10.0, 8.5, 230.0, 172.0),
                radius,
            });
            clip
        };
        for clip in [
            Bounds::new(0.0, 0.0, 240.0, 180.0),
            Bounds::new(30.0, 25.0, 190.0, 140.0),
        ]
        .map(|bounds| Clip {
            bounds,
            rounded: None,
        })
        .into_iter()
        .chain([rounded(16.0), rounded(500.0)])
        {
            let list = big_quads(clip.clone());
            let mut full = Rasterizer::new(240, 180);
            full.render(&list, &Damage::Full);
            let mut slow = Rasterizer::new(240, 180);
            slow.solid_fast_path = false;
            slow.render(&list, &Damage::Full);
            assert_eq!(
                first_difference(full.pixmap().data(), slow.pixmap().data(), 240),
                None,
                "clip {clip:?}"
            );
            let mut partial = Rasterizer::new(240, 180);
            partial.render(&list, &Damage::Full);
            partial.render(
                &list,
                &Damage::Partial(vec![
                    Bounds::new(0.0, 0.0, 100.0, 64.0),
                    Bounds::new(150.0, 90.0, 240.0, 180.0),
                ]),
            );
            assert_eq!(
                first_difference(partial.pixmap().data(), full.pixmap().data(), 240),
                None,
                "clip {clip:?}"
            );
        }
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
    fn radial_gradient_interpolates_from_center_and_clips_corners() {
        let list = record(|p| {
            p.fill_radial_gradient(
                rect(0.0, 0.0, 40.0, 30.0),
                Color::rgb(255, 0, 0),
                Color::rgb(0, 0, 255),
                creamui_core::Point { x: 20.0, y: 15.0 },
                25.0,
                8.0,
            )
        });
        assert_eq!(list.items.len(), 1);
        let mut raster = Rasterizer::new(40, 30);
        raster.render(&list, &Damage::Full);
        let center = rgba(&raster, 20, 15);
        let edge = rgba(&raster, 38, 15);
        assert!(center[0] > 240 && center[2] < 15);
        assert!(edge[2] > edge[0]);
        assert_eq!(rgba(&raster, 0, 0), [10, 20, 30, 255]);
    }

    #[test]
    fn elliptical_gradient_uses_independent_radii() {
        let list = record(|p| {
            p.fill_radial_gradient_ellipse(
                rect(0.0, 0.0, 40.0, 30.0),
                Color::rgb(255, 0, 0),
                Color::rgb(0, 0, 255),
                creamui_core::Point { x: 20.0, y: 15.0 },
                [15.0, 5.0],
                0.0,
            )
        });
        let mut raster = Rasterizer::new(40, 30);
        raster.render(&list, &Damage::Full);
        let horizontal = rgba(&raster, 30, 15);
        let vertical = rgba(&raster, 20, 25);
        assert!(horizontal[0] > 50 && horizontal[2] > horizontal[0]);
        assert_eq!(vertical, [0, 0, 255, 255]);
    }

    #[test]
    fn repeating_radial_gradient_restarts_after_each_radius() {
        let list = record(|p| {
            p.fill_radial_gradient_ellipse_repeating(
                rect(0.0, 0.0, 40.0, 30.0),
                Color::rgb(255, 0, 0),
                Color::rgb(0, 0, 255),
                creamui_core::Point { x: 0.5, y: 15.5 },
                [10.0, 10.0],
                true,
                0.0,
            )
        });
        let mut raster = Rasterizer::new(40, 30);
        raster.render(&list, &Damage::Full);
        for channel in 0..4 {
            assert!(rgba(&raster, 5, 15)[channel].abs_diff(rgba(&raster, 15, 15)[channel]) <= 2);
        }
    }

    #[test]
    fn radial_gradient_interpolates_multiple_stops() {
        let stops = [
            ResolvedRadialColorStop {
                offset: 0.0,
                color: Color::rgb(255, 0, 0),
            },
            ResolvedRadialColorStop {
                offset: 0.5,
                color: Color::rgb(0, 255, 0),
            },
            ResolvedRadialColorStop {
                offset: 1.0,
                color: Color::rgb(0, 0, 255),
            },
        ];
        let list = record(|p| {
            p.fill_radial_gradient_stops(
                rect(0.0, 0.0, 40.0, 30.0),
                &stops,
                creamui_core::Point { x: 0.5, y: 15.5 },
                [20.0, 20.0],
                false,
                0.0,
            )
        });
        let mut raster = Rasterizer::new(40, 30);
        raster.render(&list, &Damage::Full);
        assert_eq!(rgba(&raster, 0, 15), [255, 0, 0, 255]);
        assert_eq!(rgba(&raster, 10, 15), [0, 255, 0, 255]);
        assert_eq!(rgba(&raster, 20, 15), [0, 0, 255, 255]);
    }

    #[test]
    fn repeating_radial_stops_use_their_own_interval() {
        let stops = [
            ResolvedRadialColorStop {
                offset: 0.2,
                color: Color::rgb(255, 0, 0),
            },
            ResolvedRadialColorStop {
                offset: 0.4,
                color: Color::rgb(0, 255, 0),
            },
            ResolvedRadialColorStop {
                offset: 0.6,
                color: Color::rgb(0, 0, 255),
            },
        ];
        let list = record(|p| {
            p.fill_radial_gradient_stops(
                rect(0.0, 0.0, 40.0, 30.0),
                &stops,
                creamui_core::Point { x: 0.5, y: 15.5 },
                [10.0, 10.0],
                true,
                0.0,
            )
        });
        let mut raster = Rasterizer::new(40, 30);
        raster.render(&list, &Damage::Full);
        assert_eq!(rgba(&raster, 4, 15), [0, 255, 0, 255]);
        let first_period = rgba(&raster, 3, 15);
        let next_period = rgba(&raster, 7, 15);
        assert!(first_period[0] > 100 && first_period[1] > 100);
        assert!(first_period
            .iter()
            .zip(next_period)
            .all(|(a, b)| a.abs_diff(b) <= 1));
    }

    #[test]
    fn inside_border_keeps_its_outer_edge_within_the_layout_box() {
        let list = record(|p| {
            p.fill_rect(rect(5.0, 5.0, 20.0, 20.0), Color::rgb(255, 255, 255), 0.0);
            p.stroke_rect_inside(rect(5.0, 5.0, 20.0, 20.0), Color::rgb(255, 0, 0), 4.0, 0.0);
        });
        let mut raster = Rasterizer::new(40, 30);
        raster.render(&list, &Damage::Full);
        assert_eq!(rgba(&raster, 4, 15), [10, 20, 30, 255]);
        assert_eq!(rgba(&raster, 5, 15), [255, 0, 0, 255]);
        assert_eq!(rgba(&raster, 10, 15), [255, 255, 255, 255]);
    }

    #[test]
    fn radial_gradient_transparency_uses_premultiplied_colors() {
        let mut recorder = SceneRecorder::new();
        recorder.begin(40, 30, 1.0, Color::rgba(0, 0, 0, 0), ColorScheme::default());
        recorder.fill_radial_gradient(
            rect(0.0, 0.0, 40.0, 30.0),
            Color::rgb(255, 0, 0),
            Color::rgba(0, 0, 255, 0),
            creamui_core::Point { x: 0.5, y: 15.5 },
            20.0,
            0.0,
        );
        let mut raster = Rasterizer::new(40, 30);
        raster.render(&recorder.finish(), &Damage::Full);
        assert_eq!(rgba(&raster, 10, 15), [128, 0, 0, 128]);
        assert_eq!(rgba(&raster, 30, 15), [0, 0, 0, 0]);
    }

    #[test]
    fn linear_gradient_transparency_uses_premultiplied_colors() {
        let mut recorder = SceneRecorder::new();
        recorder.begin(40, 30, 1.0, Color::rgba(0, 0, 0, 0), ColorScheme::default());
        recorder.fill_linear_gradient(
            rect(0.0, 0.0, 40.0, 30.0),
            Color::rgb(255, 0, 0),
            Color::rgba(0, 0, 255, 0),
            90.0,
            0.0,
        );
        let mut raster = Rasterizer::new(40, 30);
        raster.render(&recorder.finish(), &Damage::Full);
        assert_eq!(rgba(&raster, 19, 15), [131, 0, 0, 131]);
        assert_eq!(rgba(&raster, 39, 15), [3, 0, 0, 3]);
    }

    #[test]
    fn radial_gradient_partial_repaint_matches_full_with_clipping() {
        for end_alpha in [255, 40] {
            let frame = |center_x| {
                record(|p| {
                    p.push_clip_rounded(rect(6.0, 4.0, 28.0, 22.0), 6.0);
                    p.fill_radial_gradient(
                        rect(0.0, 0.0, 40.0, 30.0),
                        Color::rgb(255, 0, 0),
                        Color::rgba(0, 0, 255, end_alpha),
                        creamui_core::Point {
                            x: center_x,
                            y: 15.0,
                        },
                        25.0,
                        4.0,
                    );
                    p.pop_clip();
                })
            };
            let before = frame(10.0);
            let after = frame(30.0);
            assert!(!matches!(damage(Some(&before), &after), Damage::None));
            let mut partial = Rasterizer::new(40, 30);
            partial.render(&before, &Damage::Full);
            partial.render(
                &after,
                &Damage::Partial(vec![Bounds::new(0.0, 0.0, 40.0, 15.0)]),
            );
            partial.render(
                &after,
                &Damage::Partial(vec![Bounds::new(0.0, 15.0, 40.0, 30.0)]),
            );
            let mut full = Rasterizer::new(40, 30);
            full.render(&after, &Damage::Full);
            assert_eq!(partial.pixmap().data(), full.pixmap().data());
        }
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
    fn nested_rounded_clips_keep_outer_corners() {
        let list = record(|p| {
            p.push_clip_rounded(rect(1.0, 1.0, 28.0, 28.0), 10.0);
            p.push_clip_rounded(rect(1.0, 1.0, 28.0, 28.0), 3.0);
            p.fill_rect(rect(0.0, 0.0, 30.0, 30.0), Color::rgb(255, 0, 0), 0.0);
            p.pop_clip();
            p.pop_clip();
        });
        let mut raster = Rasterizer::new(40, 30);
        raster.render(&list, &Damage::Full);
        assert_eq!(rgba(&raster, 3, 3), [10, 20, 30, 255]);
        assert_eq!(rgba(&raster, 15, 15), [255, 0, 0, 255]);
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
    fn large_tinted_thumbnails_do_not_cache_full_size_pixels() {
        let mut pixels = vec![0; 1024 * 1024 * 4];
        for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
            let alpha = (index % 253) as u8;
            pixel.copy_from_slice(&[alpha / 2, alpha / 3, alpha / 4, alpha]);
        }
        let image = RgbaImage::new(1024, 1024, pixels).unwrap();
        let tint = Color::rgba(35, 140, 225, 180);
        let effects = creamui_core::ImageEffects {
            quarter_turns: 1,
            color_matrix: [
                [0.5, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
        };
        let list = record(|p| {
            p.push_clip_rounded(rect(5.0, 4.0, 30.0, 22.0), 5.0);
            p.draw_image_effects(rect(4.0, 3.0, 32.0, 24.0), &image, Some(tint), 0.8, effects);
            p.pop_clip();
        });
        let mut raster = Rasterizer::new(40, 30);
        raster.render(&list, &Damage::Full);
        assert!(raster.images.tinted.is_empty());
        assert_eq!(raster.images.scaled.len(), 1);
        assert!(raster.images.scaled_bytes <= 40 * 30 * 4);
        let first = raster.pixmap().data().to_vec();
        raster.render(&list, &Damage::Full);
        assert_eq!(raster.pixmap().data(), first);

        let mut pre_tinted = image.pixels().to_vec();
        for pixel in pre_tinted.chunks_exact_mut(4) {
            let alpha = (pixel[3] as u32 * tint.a as u32 + 127) / 255;
            pixel.copy_from_slice(&[
                mul(tint.r, alpha),
                mul(tint.g, alpha),
                mul(tint.b, alpha),
                alpha as u8,
            ]);
        }
        let pre_tinted = RgbaImage::new(1024, 1024, pre_tinted).unwrap();
        let reference = record(|p| {
            p.push_clip_rounded(rect(5.0, 4.0, 30.0, 22.0), 5.0);
            p.draw_image_effects(rect(4.0, 3.0, 32.0, 24.0), &pre_tinted, None, 0.8, effects);
            p.pop_clip();
        });
        let mut expected = Rasterizer::new(40, 30);
        expected.render(&reference, &Damage::Full);
        let max_difference = raster
            .pixmap()
            .data()
            .iter()
            .zip(expected.pixmap().data())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(
            max_difference <= 2,
            "max pixel difference: {max_difference}"
        );
    }

    #[test]
    fn scaled_tint_cache_evicts_old_entries_at_its_byte_limit() {
        let mut cache = TintCache::default();
        let key = |image| ScaledTintKey {
            image,
            tint: Color::rgb(1, 2, 3),
            transform: [0; 6],
            crop: [0; 4],
            matrix: [0; 12],
        };
        for image in 0..3 {
            cache.frame = image;
            cache.store_scaled(key(image), Pixmap::new(1024, 1024).unwrap());
        }
        assert_eq!(cache.scaled.len(), 2);
        assert_eq!(cache.scaled_bytes, SCALED_TINT_CACHE_BYTES);
        assert!(!cache.scaled.contains_key(&key(0)));
        cache.store_scaled(key(3), Pixmap::new(2048, 2048).unwrap());
        assert_eq!(cache.scaled.len(), 2);
    }

    #[test]
    fn resizing_forces_a_full_repaint() {
        let list = record(|_| {});
        let mut r = Rasterizer::new(10, 10);
        assert_eq!(r.render(&list, &Damage::None), Damage::Full);
        assert_eq!((r.pixmap().width(), r.pixmap().height()), (40, 30));
    }

    #[test]
    fn shrinking_the_frame_reclaims_oversized_clip_scratch() {
        let mut recorder = SceneRecorder::new();
        recorder.begin(
            1024,
            1024,
            1.0,
            Color::rgb(10, 20, 30),
            ColorScheme::default(),
        );
        recorder.push_clip_rounded(rect(0.0, 0.0, 1024.0, 1024.0), 16.0);
        recorder.fill_linear_gradient(
            rect(0.0, 0.0, 1024.0, 1024.0),
            Color::rgb(255, 0, 0),
            Color::rgb(0, 0, 255),
            90.0,
            16.0,
        );
        recorder.pop_clip();
        let mut raster = Rasterizer::new(1024, 1024);
        raster.render(&recorder.finish(), &Damage::Full);
        assert!(raster.scratch.capacity() >= 1024 * 1024 * 4);

        let mut recorder = SceneRecorder::new();
        recorder.begin(64, 64, 1.0, Color::rgb(10, 20, 30), ColorScheme::default());
        raster.render(&recorder.finish(), &Damage::None);
        assert!(raster.scratch.capacity() <= 64 * 64 * 4);
        assert_eq!(rgba(&raster, 0, 0), [10, 20, 30, 255]);
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
