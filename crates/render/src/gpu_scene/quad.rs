use bytemuck::Zeroable;
use creamui_core::runtime::{PaintFragment, PaintOp, PaintPrimitive};
use creamui_theme::Color;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
    pub border_color: [f32; 4],
    pub corner_radius: f32,
    pub border_width: f32,
    padding: [f32; 2],
}

impl QuadInstance {
    pub fn fill(
        rect: creamui_core::Rect,
        color: Color,
        corner_radius: f32,
        decode_srgb: bool,
    ) -> Self {
        QuadInstance {
            position: [rect.x, rect.y],
            size: [rect.width, rect.height],
            color: quad_color(color, decode_srgb),
            border_color: [0.0; 4],
            corner_radius,
            border_width: 0.0,
            padding: [0.0; 2],
        }
    }

    pub fn border(
        rect: creamui_core::Rect,
        color: Color,
        width: f32,
        corner_radius: f32,
        decode_srgb: bool,
    ) -> Self {
        QuadInstance {
            position: [rect.x, rect.y],
            size: [rect.width, rect.height],
            color: [0.0; 4],
            border_color: quad_color(color, decode_srgb),
            corner_radius,
            border_width: width,
            padding: [0.0; 2],
        }
    }
}

fn srgb_channel_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// `decode_srgb` must be `true` when the render target applies its own
/// linear -> sRGB encoding on write (an `Srgb` surface format), so the
/// stored instance color is linear and the round trip reproduces `color`
/// on screen instead of re-encoding an already-encoded value.
pub fn quad_color(color: Color, decode_srgb: bool) -> [f32; 4] {
    let [r, g, b, a] = color.to_f32();
    if decode_srgb {
        [
            srgb_channel_to_linear(r),
            srgb_channel_to_linear(g),
            srgb_channel_to_linear(b),
            a,
        ]
    } else {
        [r, g, b, a]
    }
}

/// Translates a retained [`PaintFragment`]'s quad/border primitives into
/// GPU quad instances. Text and image primitives are not part of the GPU
/// quad pipeline yet (REFACTOR.md 14.8's migration order handles those
/// after solid quads, borders, and rounded corners); clip/transform ops
/// are likewise not yet consumed here.
pub fn quad_instances_for_fragment(
    fragment: &PaintFragment,
    decode_srgb: bool,
) -> Vec<QuadInstance> {
    fragment
        .ops
        .iter()
        .filter_map(|op| match op {
            PaintOp::Primitive(PaintPrimitive::Quad(quad)) => Some(QuadInstance::fill(
                quad.rect,
                quad.color,
                quad.corner_radius,
                decode_srgb,
            )),
            PaintOp::Primitive(PaintPrimitive::Border(border)) => Some(QuadInstance::border(
                border.rect,
                border.color,
                border.width,
                border.corner_radius,
                decode_srgb,
            )),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuPrimitiveId(u32);

/// A free-list-backed store of [`QuadInstance`]s addressed by stable
/// [`GpuPrimitiveId`]s, tracking the smallest contiguous index range
/// touched since the last [`QuadStore::take_dirty_range`] call so a
/// caller can patch only that range of a GPU buffer instead of
/// re-uploading every instance.
#[derive(Default)]
pub struct QuadStore {
    slots: Vec<QuadInstance>,
    free: Vec<u32>,
    dirty: Option<(u32, u32)>,
}

impl QuadStore {
    pub fn new() -> Self {
        QuadStore::default()
    }

    pub fn insert(&mut self, instance: QuadInstance) -> GpuPrimitiveId {
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
        GpuPrimitiveId(index)
    }

    pub fn update(&mut self, id: GpuPrimitiveId, instance: QuadInstance) {
        let slot = &mut self.slots[id.0 as usize];
        if *slot != instance {
            *slot = instance;
            self.mark_dirty(id.0);
        }
    }

    pub fn remove(&mut self, id: GpuPrimitiveId) {
        self.slots[id.0 as usize] = QuadInstance::zeroed();
        self.free.push(id.0);
        self.mark_dirty(id.0);
    }

    pub fn instances(&self) -> &[QuadInstance] {
        &self.slots
    }

    pub fn take_dirty_range(&mut self) -> Option<(u32, u32)> {
        self.dirty.take()
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
    use creamui_core::runtime::{BorderPrimitive, QuadPrimitive};
    use creamui_core::Rect;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn insert_returns_distinct_ids_and_marks_dirty() {
        let mut store = QuadStore::new();
        let a = store.insert(QuadInstance::zeroed());
        let b = store.insert(QuadInstance::zeroed());
        assert_ne!(a, b);
        assert_eq!(store.take_dirty_range(), Some((0, 1)));
    }

    #[test]
    fn update_with_unchanged_instance_does_not_mark_dirty() {
        let mut store = QuadStore::new();
        let instance =
            QuadInstance::fill(rect(0.0, 0.0, 10.0, 10.0), Color::rgb(1, 2, 3), 0.0, false);
        let id = store.insert(instance);
        store.take_dirty_range();

        store.update(id, instance);
        assert_eq!(store.take_dirty_range(), None);
    }

    #[test]
    fn update_with_changed_instance_marks_only_that_slot_dirty() {
        let mut store = QuadStore::new();
        let a = store.insert(QuadInstance::zeroed());
        let _b = store.insert(QuadInstance::zeroed());
        store.take_dirty_range();

        store.update(
            a,
            QuadInstance::fill(rect(0.0, 0.0, 1.0, 1.0), Color::rgb(9, 9, 9), 0.0, false),
        );
        assert_eq!(store.take_dirty_range(), Some((0, 0)));
    }

    #[test]
    fn remove_reuses_the_freed_slot_on_next_insert() {
        let mut store = QuadStore::new();
        let a = store.insert(QuadInstance::zeroed());
        let _b = store.insert(QuadInstance::zeroed());
        store.remove(a);

        let c = store.insert(QuadInstance::fill(
            rect(0.0, 0.0, 1.0, 1.0),
            Color::rgb(1, 1, 1),
            0.0,
            false,
        ));
        assert_eq!(c, a);
        assert_eq!(store.instances().len(), 2);
    }

    #[test]
    fn dirty_range_spans_the_lowest_and_highest_touched_index() {
        let mut store = QuadStore::new();
        let a = store.insert(QuadInstance::zeroed());
        let _b = store.insert(QuadInstance::zeroed());
        let c = store.insert(QuadInstance::zeroed());
        store.take_dirty_range();

        store.update(
            c,
            QuadInstance::fill(rect(0.0, 0.0, 1.0, 1.0), Color::rgb(1, 1, 1), 0.0, false),
        );
        store.update(
            a,
            QuadInstance::fill(rect(0.0, 0.0, 2.0, 2.0), Color::rgb(2, 2, 2), 0.0, false),
        );
        assert_eq!(store.take_dirty_range(), Some((0, 2)));
    }

    #[test]
    fn quad_color_identity_when_not_decoding_srgb() {
        let color = Color::rgba(255, 128, 0, 64);
        assert_eq!(quad_color(color, false), color.to_f32());
    }

    #[test]
    fn quad_color_decodes_srgb_endpoints_exactly() {
        assert_eq!(quad_color(Color::rgb(0, 0, 0), true), [0.0, 0.0, 0.0, 1.0]);
        let [r, g, b, a] = quad_color(Color::rgb(255, 255, 255), true);
        assert!((r - 1.0).abs() < 1e-5);
        assert!((g - 1.0).abs() < 1e-5);
        assert!((b - 1.0).abs() < 1e-5);
        assert_eq!(a, 1.0);
    }

    #[test]
    fn quad_color_decoding_darkens_midtones() {
        let [r, ..] = quad_color(Color::rgb(128, 128, 128), true);
        assert!(r < 128.0 / 255.0);
    }

    #[test]
    fn fragment_translation_keeps_only_quad_and_border_primitives() {
        let fragment = PaintFragment {
            ops: vec![
                PaintOp::PushClip(rect(0.0, 0.0, 100.0, 100.0)),
                PaintOp::Primitive(PaintPrimitive::Quad(QuadPrimitive {
                    rect: rect(0.0, 0.0, 10.0, 10.0),
                    color: Color::rgb(1, 2, 3),
                    corner_radius: 4.0,
                })),
                PaintOp::Primitive(PaintPrimitive::Border(BorderPrimitive {
                    rect: rect(0.0, 0.0, 10.0, 10.0),
                    color: Color::rgb(4, 5, 6),
                    width: 2.0,
                    corner_radius: 4.0,
                })),
                PaintOp::PopClip,
            ],
            bounds: rect(0.0, 0.0, 10.0, 10.0),
        };

        let instances = quad_instances_for_fragment(&fragment, false);
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].border_width, 0.0);
        assert_eq!(instances[1].border_width, 2.0);
    }
}
