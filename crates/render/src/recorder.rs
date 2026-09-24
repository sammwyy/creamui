//! [`Painter`] implementation that records widgets' draw calls into a
//! [`DisplayList`] in physical pixels instead of rasterizing them.

use crate::display_list::{
    Bounds, Clip, DisplayList, DrawItem, ImagePrimitive, Line, Primitive, Quad, QuadGradient,
    RoundedClip, ScrollLayer, TextRun,
};
use crate::text::TextSystem;
use creamui_core::{Painter, Point, Rect, RgbaImage, TextAlign};
use creamui_theme::{Color, ColorScheme};
use std::cell::{Ref, RefCell};
use std::ops::Range;
use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
type RecorderInstant = std::time::Instant;
#[cfg(target_arch = "wasm32")]
type RecorderInstant = web_time::Instant;

const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
/// The clip of content directly inside a scroll layer, whose visible part
/// the layer's viewport decides.
const UNBOUNDED: Bounds = Bounds {
    x0: -1.0e7,
    y0: -1.0e7,
    x1: 1.0e7,
    y1: 1.0e7,
};

/// Content coordinates are rounded to 1/256 px so the same content maps
/// to bit-identical items at any scroll offset.
fn snap(value: f32) -> f32 {
    (value * 256.0).round() / 256.0
}

#[derive(Clone, Copy)]
struct ClipState {
    /// Recorded with items, in the current layer's content space.
    clip: Clip,
    /// What is visible on screen, for culling.
    visible: Bounds,
    layer: u16,
    /// Screen position of the current layer's content origin.
    translation: [f32; 2],
}

pub struct SceneRecorder {
    text: Rc<RefCell<TextSystem>>,
    list: DisplayList,
    spare: Vec<DrawItem>,
    clips: Vec<ClipState>,
    scale: f32,
    color_scheme: ColorScheme,
    pub pointer: Option<Point>,
    pub press_origin: Option<Point>,
    animated: bool,
    started: RecorderInstant,
    frame_time: f32,
}

impl Default for SceneRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneRecorder {
    pub fn new() -> Self {
        SceneRecorder {
            text: TextSystem::shared(),
            list: DisplayList::new(1, 1, TRANSPARENT),
            spare: Vec::new(),
            clips: Vec::new(),
            scale: 1.0,
            color_scheme: ColorScheme::default(),
            pointer: None,
            press_origin: None,
            animated: false,
            started: RecorderInstant::now(),
            frame_time: 0.0,
        }
    }

    /// Starts a frame of `width` x `height` physical pixels. Widgets paint in
    /// logical pixels, scaled by `scale`.
    pub fn begin(
        &mut self,
        width: u32,
        height: u32,
        scale: f32,
        clear: Color,
        color_scheme: ColorScheme,
    ) {
        let mut items = std::mem::take(&mut self.spare);
        items.clear();
        self.list = DisplayList {
            width: width.max(1),
            height: height.max(1),
            clear,
            items,
            layers: Vec::new(),
        };
        self.scale = scale.max(0.01);
        self.color_scheme = color_scheme;
        self.animated = false;
        self.frame_time = self.started.elapsed().as_secs_f32();
        self.clips.clear();
        self.clips.push(ClipState {
            clip: Clip {
                bounds: self.list.viewport(),
                rounded: None,
            },
            visible: self.list.viewport(),
            layer: 0,
            translation: [0.0; 2],
        });
    }

    /// Ends the frame started by [`SceneRecorder::begin`].
    pub fn finish(&mut self) -> DisplayList {
        self.text.borrow_mut().end_frame();
        std::mem::replace(&mut self.list, DisplayList::new(1, 1, TRANSPARENT))
    }

    /// Hands a no-longer-needed list back so its allocation is reused.
    pub fn recycle(&mut self, list: DisplayList) {
        if list.items.capacity() > self.spare.capacity() {
            self.spare = list.items;
        }
    }

    /// Whether any widget asked for animation frames since
    /// [`SceneRecorder::begin`].
    pub fn animated(&self) -> bool {
        self.animated
    }

    pub fn text(&self) -> Ref<'_, TextSystem> {
        self.text.borrow()
    }

    fn state(&self) -> ClipState {
        *self.clips.last().expect("begin pushes the viewport clip")
    }

    fn visible(&self, bounds: Bounds) -> bool {
        let state = self.state();
        !bounds
            .translate(state.translation)
            .intersect(state.visible)
            .is_empty()
    }

    fn push(&mut self, primitive: Primitive) {
        if !self.visible(primitive.bounds()) {
            return;
        }
        #[cfg(feature = "perf-metrics")]
        creamui_core::metrics::record(|m| m.display_items += 1);
        let state = self.state();
        self.list.items.push(DrawItem {
            primitive,
            clip: state.clip,
            layer: state.layer,
        });
    }

    /// `rect`, in logical screen pixels, in the current layer's physical
    /// content space.
    fn bounds(&self, rect: Rect) -> Bounds {
        let screen = Bounds::from_rect(rect, self.scale);
        let [x, y] = self.state().translation;
        if self.state().layer == 0 {
            return screen;
        }
        Bounds {
            x0: snap(screen.x0 - x),
            y0: snap(screen.y0 - y),
            x1: snap(screen.x1 - x),
            y1: snap(screen.y1 - y),
        }
    }

    fn point(&self, point: Point) -> [f32; 2] {
        let [x, y] = self.state().translation;
        let (px, py) = (point.x * self.scale, point.y * self.scale);
        if self.state().layer == 0 {
            return [px, py];
        }
        [snap(px - x), snap(py - y)]
    }

    #[allow(clippy::too_many_arguments)]
    fn text_run(
        &mut self,
        rect: Rect,
        text: &str,
        color: Color,
        selection: Option<(Range<usize>, Color)>,
        font_size: f32,
        align: TextAlign,
        family: Option<&str>,
        bold: bool,
        italic: bool,
    ) {
        if text.is_empty() || (color.a == 0 && selection.is_none()) {
            return;
        }
        let bounds = self.bounds(rect);
        if !self.visible(bounds) {
            return;
        }
        let layout = self.text.borrow_mut().layout(
            family,
            bold,
            text,
            font_size * self.scale,
            bounds.width().max(0.0),
            align,
        );
        if layout.glyphs.is_empty() {
            return;
        }
        let top = ((bounds.height() - layout.height) * 0.5).floor() as i32;
        self.push(Primitive::Text(TextRun {
            layout,
            x: bounds.x0.round() as i32,
            y: bounds.y0.round() as i32 + top,
            color,
            selection,
            italic,
        }));
    }
}

impl Painter for SceneRecorder {
    fn color_scheme(&self) -> ColorScheme {
        self.color_scheme
    }

    fn hovered(&self, rect: Rect) -> bool {
        self.pointer.is_some_and(|point| rect.contains(point))
    }

    fn pressed(&self, rect: Rect) -> bool {
        self.hovered(rect) && self.press_origin.is_some_and(|point| rect.contains(point))
    }

    /// The same instant for every widget in a frame, so animations stay in
    /// step with each other however long the frame takes to record.
    fn animation_time(&mut self) -> f32 {
        self.animated = true;
        self.frame_time
    }

    fn stroke_line(&mut self, from: Point, to: Point, color: Color, width: f32) {
        if color.a == 0 || width <= 0.0 {
            return;
        }
        self.push(Primitive::Line(Line {
            from: self.point(from),
            to: self.point(to),
            width: width * self.scale,
            color,
        }));
    }

    fn fill_rect(&mut self, rect: Rect, color: Color, corner_radius: f32) {
        let bounds = self.bounds(rect);
        if color.a == 0 || bounds.is_empty() {
            return;
        }
        self.push(Primitive::Quad(Quad {
            bounds,
            background: color,
            gradient: None,
            radius: (corner_radius * self.scale)
                .min(bounds.width() / 2.0)
                .min(bounds.height() / 2.0)
                .max(0.0),
            border_width: 0.0,
            border_color: TRANSPARENT,
        }));
    }

    fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        start: Color,
        end: Color,
        angle_degrees: f32,
        corner_radius: f32,
    ) {
        let bounds = self.bounds(rect);
        if (start.a == 0 && end.a == 0) || bounds.is_empty() {
            return;
        }
        let radians = angle_degrees.to_radians();
        let direction = [radians.sin(), -radians.cos()];
        let half =
            direction[0].abs() * bounds.width() * 0.5 + direction[1].abs() * bounds.height() * 0.5;
        let center = [(bounds.x0 + bounds.x1) * 0.5, (bounds.y0 + bounds.y1) * 0.5];
        self.push(Primitive::Quad(Quad {
            bounds,
            background: start,
            gradient: Some(QuadGradient {
                start: [
                    center[0] - direction[0] * half,
                    center[1] - direction[1] * half,
                ],
                end: [
                    center[0] + direction[0] * half,
                    center[1] + direction[1] * half,
                ],
                start_color: start,
                end_color: end,
            }),
            radius: (corner_radius * self.scale)
                .min(bounds.width() / 2.0)
                .min(bounds.height() / 2.0)
                .max(0.0),
            border_width: 0.0,
            border_color: TRANSPARENT,
        }));
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32, corner_radius: f32) {
        if color.a == 0 || width <= 0.0 {
            return;
        }
        let width = width * self.scale;
        let bounds = self.bounds(rect).inflate(width / 2.0);
        if bounds.is_empty() {
            return;
        }
        let radius = if corner_radius > 0.0 {
            (corner_radius * self.scale + width / 2.0)
                .min(bounds.width() / 2.0)
                .min(bounds.height() / 2.0)
        } else {
            0.0
        };
        self.push(Primitive::Quad(Quad {
            bounds,
            background: TRANSPARENT,
            gradient: None,
            radius,
            border_width: width,
            border_color: color,
        }));
    }

    fn fill_text(
        &mut self,
        rect: Rect,
        text: &str,
        color: Color,
        font_size: f32,
        align: TextAlign,
    ) {
        self.text_run(
            rect, text, color, None, font_size, align, None, false, false,
        );
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
        self.text_run(
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
        self.text_run(
            rect, text, color, None, font_size, align, family, bold, italic,
        );
    }

    fn fill_text_selected(
        &mut self,
        rect: Rect,
        text: &str,
        color: Color,
        selected_color: Color,
        selected: Range<usize>,
        font_size: f32,
        align: TextAlign,
    ) {
        self.text_run(
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
        selected: Range<usize>,
        font_size: f32,
        align: TextAlign,
        family: Option<&str>,
    ) {
        self.text_run(
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

    fn draw_image(&mut self, rect: Rect, image: &RgbaImage, tint: Option<Color>) {
        let bounds = self.bounds(rect);
        if bounds.is_empty() || tint.is_some_and(|tint| tint.a == 0) {
            return;
        }
        self.push(Primitive::Image(ImagePrimitive {
            bounds,
            image: image.clone(),
            tint,
        }));
    }

    fn push_clip(&mut self, rect: Rect) {
        self.push_clip_rounded(rect, 0.0);
    }

    fn push_clip_rounded(&mut self, rect: Rect, corner_radius: f32) {
        let parent = self.state();
        let bounds = self.bounds(rect).round();
        let rounded = if corner_radius > 0.0 {
            Some(RoundedClip {
                bounds,
                radius: (corner_radius * self.scale)
                    .min(bounds.width() / 2.0)
                    .min(bounds.height() / 2.0)
                    .max(0.0),
            })
        } else {
            parent.clip.rounded
        };
        let clipped = parent.clip.bounds.intersect(bounds);
        self.clips.push(ClipState {
            clip: Clip {
                bounds: clipped,
                rounded: rounded.filter(|r| !r.inner().contains(clipped)),
            },
            visible: parent
                .visible
                .intersect(bounds.translate(parent.translation)),
            ..parent
        });
    }

    fn pop_clip(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
        }
    }

    fn push_scroll_layer(&mut self, viewport: Rect, corner_radius: f32, offset: Point) {
        let parent = self.state();
        let viewport = self.bounds(viewport).round();
        let offset = [-offset.x * self.scale, -offset.y * self.scale];
        self.list.layers.push(ScrollLayer {
            parent: parent.layer,
            viewport,
            radius: (corner_radius * self.scale)
                .min(viewport.width() / 2.0)
                .min(viewport.height() / 2.0)
                .max(0.0),
            offset,
        });
        self.clips.push(ClipState {
            clip: Clip {
                bounds: UNBOUNDED,
                rounded: None,
            },
            visible: parent
                .visible
                .intersect(viewport.translate(parent.translation)),
            layer: self.list.layers.len() as u16,
            translation: [
                parent.translation[0] + offset[0],
                parent.translation[1] + offset[1],
            ],
        });
    }

    fn pop_scroll_layer(&mut self) {
        self.pop_clip();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn record(scale: f32, paint: impl FnOnce(&mut SceneRecorder)) -> DisplayList {
        let mut recorder = SceneRecorder::new();
        recorder.begin(200, 100, scale, Color::rgb(0, 0, 0), ColorScheme::default());
        paint(&mut recorder);
        recorder.finish()
    }

    #[test]
    fn records_in_physical_pixels() {
        let list = record(2.0, |p| {
            p.fill_rect(rect(1.0, 2.0, 3.0, 4.0), Color::rgb(1, 2, 3), 1.0)
        });
        let Primitive::Quad(quad) = &list.items[0].primitive else {
            panic!("expected a quad");
        };
        assert_eq!(quad.bounds, Bounds::new(2.0, 4.0, 8.0, 12.0));
        assert_eq!(quad.radius, 2.0);
    }

    #[test]
    fn strokes_are_centered_on_the_rect_edge() {
        let list = record(1.0, |p| {
            p.stroke_rect(rect(10.0, 10.0, 20.0, 20.0), Color::rgb(1, 2, 3), 4.0, 6.0)
        });
        let Primitive::Quad(quad) = &list.items[0].primitive else {
            panic!("expected a quad");
        };
        assert_eq!(quad.bounds, Bounds::new(8.0, 8.0, 32.0, 32.0));
        assert_eq!(quad.radius, 8.0);
        assert_eq!(quad.border_width, 4.0);
        assert_eq!(quad.background.a, 0);
    }

    #[test]
    fn invisible_and_fully_clipped_draws_are_dropped() {
        let list = record(1.0, |p| {
            p.fill_rect(rect(0.0, 0.0, 10.0, 10.0), Color::rgba(1, 2, 3, 0), 0.0);
            p.fill_rect(rect(500.0, 0.0, 10.0, 10.0), Color::rgb(1, 2, 3), 0.0);
            p.push_clip(rect(0.0, 0.0, 10.0, 10.0));
            p.fill_rect(rect(20.0, 20.0, 10.0, 10.0), Color::rgb(1, 2, 3), 0.0);
            p.pop_clip();
            p.fill_text(
                rect(0.0, 0.0, 50.0, 20.0),
                "",
                Color::rgb(1, 2, 3),
                12.0,
                TextAlign::Start,
            );
        });
        assert!(list.items.is_empty());
    }

    #[test]
    fn nested_clips_intersect_and_keep_the_innermost_rounding() {
        let list = record(1.0, |p| {
            p.push_clip_rounded(rect(0.0, 0.0, 100.0, 100.0), 10.0);
            p.push_clip(rect(50.0, -20.0, 100.0, 60.0));
            p.fill_rect(rect(0.0, 0.0, 200.0, 200.0), Color::rgb(1, 2, 3), 0.0);
            p.pop_clip();
            p.pop_clip();
            p.fill_rect(rect(0.0, 0.0, 5.0, 5.0), Color::rgb(1, 2, 3), 0.0);
        });
        let clip = list.items[0].clip;
        assert_eq!(clip.bounds, Bounds::new(50.0, 0.0, 100.0, 40.0));
        assert_eq!(clip.rounded.unwrap().radius, 10.0);
        assert_eq!(
            list.items[1].clip.bounds,
            Bounds::new(0.0, 0.0, 200.0, 100.0)
        );
        assert!(list.items[1].clip.rounded.is_none());
    }

    #[test]
    fn taller_boxes_reuse_the_layout_and_center_it() {
        let list = record(1.0, |p| {
            for height in [20.0, 60.0] {
                p.fill_text(
                    rect(0.0, 0.0, 100.0, height),
                    "Hi",
                    Color::rgb(9, 9, 9),
                    12.0,
                    TextAlign::Start,
                );
            }
        });
        let (Primitive::Text(short), Primitive::Text(tall)) =
            (&list.items[0].primitive, &list.items[1].primitive)
        else {
            panic!("expected two text runs");
        };
        assert!(Rc::ptr_eq(&short.layout, &tall.layout));
        assert_eq!(tall.y - short.y, 20);
    }

    #[test]
    fn repeated_text_reuses_its_layout() {
        let list = record(1.0, |p| {
            for _ in 0..2 {
                p.fill_text(
                    rect(0.0, 0.0, 100.0, 20.0),
                    "Hi",
                    Color::rgb(9, 9, 9),
                    12.0,
                    TextAlign::Start,
                );
            }
        });
        assert_eq!(list.items.len(), 2);
        assert!(list.items[0] == list.items[1]);
    }

    #[test]
    fn animation_requests_are_tracked_per_frame() {
        let mut recorder = SceneRecorder::new();
        recorder.begin(10, 10, 1.0, Color::rgb(0, 0, 0), ColorScheme::default());
        recorder.animation_time();
        assert!(recorder.animated());
        recorder.begin(10, 10, 1.0, Color::rgb(0, 0, 0), ColorScheme::default());
        assert!(!recorder.animated());
    }
}
