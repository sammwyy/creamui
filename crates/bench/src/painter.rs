//! Painters used by benchmarks: a no-op one for isolating reconcile/layout
//! cost, and [`FramePipeline`], which runs the real record, diff and CPU
//! raster stages headlessly.

use creamui_core::{Painter, Point, Rect, Renderer, TextAlign};
use creamui_render::{damage, Damage, DisplayList, Rasterizer, SceneRecorder};
use creamui_theme::{Color, ColorScheme};

/// Discards all drawing.
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

/// What one [`FramePipeline::frame`] produced.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameOutput {
    pub items: usize,
    pub damage: Damage,
    pub damaged_pixels: f32,
}

/// A window's paint pipeline without a window: records the retained tree,
/// diffs it against the previous frame and rasterizes the damage.
pub struct FramePipeline {
    pub recorder: SceneRecorder,
    pub raster: Rasterizer,
    previous: Option<DisplayList>,
    width: u32,
    height: u32,
    scale: f32,
}

impl FramePipeline {
    /// `width`/`height` are physical pixels at `scale` physical pixels per
    /// logical pixel.
    pub fn new(width: u32, height: u32, scale: f32) -> Self {
        FramePipeline {
            recorder: SceneRecorder::new(),
            raster: Rasterizer::new(width, height),
            previous: None,
            width,
            height,
            scale,
        }
    }

    pub fn set_pointer(&mut self, pointer: Option<Point>) {
        self.recorder.pointer = pointer;
    }

    /// Records `renderer`'s retained tree into a display list.
    pub fn record(&mut self, renderer: &Renderer) -> DisplayList {
        self.recorder.begin(
            self.width,
            self.height,
            self.scale,
            Color::rgb(245, 245, 248),
            ColorScheme::default(),
        );
        renderer.paint(&mut self.recorder, None, false);
        self.recorder.finish()
    }

    /// Records, diffs and rasterizes one frame.
    pub fn frame(&mut self, renderer: &Renderer) -> FrameOutput {
        let list = self.record(renderer);
        let damage = damage(self.previous.as_ref(), &list);
        let damage = self.raster.render(&list, &damage);
        let output = FrameOutput {
            items: list.items.len(),
            damaged_pixels: damage.area(list.viewport()),
            damage,
        };
        if let Some(old) = self.previous.replace(list) {
            self.recorder.recycle(old);
        }
        output
    }
}
