//! Painters used by benchmarks: a real CPU rasterizer for measuring actual
//! pixel work, and a no-op one for isolating reconcile/layout cost from
//! rasterization.

use creamui_core::{Painter, Rect, TextAlign};
use creamui_theme::Color;

/// Discards all drawing. Used by benchmarks (`tree_update`, `layout`,
/// `hover`, `scroll`) that want to isolate reconcile/layout cost from CPU
/// rasterization — see [`raster_painter`] for the real thing.
#[derive(Default)]
pub struct NoopPainter;

impl Painter for NoopPainter {
    fn fill_rect(&mut self, _rect: Rect, _color: Color, _corner_radius: f32) {}
    fn stroke_rect(&mut self, _rect: Rect, _color: Color, _width: f32, _corner_radius: f32) {}
    fn fill_text(
        &mut self,
        _rect: Rect,
        _text: &str,
        _color: Color,
        _font_size: f32,
        _align: TextAlign,
    ) {
    }
}

/// A real `tiny-skia`-backed painter (headless: no window/GPU surface
/// needed) for benchmarks that must measure actual CPU rasterization —
/// see the `paint` and `text_update` benches.
pub fn raster_painter(width: u32, height: u32) -> creamui_render::SkiaPainter {
    creamui_render::SkiaPainter::new(width, height)
}
