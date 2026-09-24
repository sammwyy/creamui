//! The persistent runtime tree: the structure meant to eventually replace
//! `Instance`/`BoxedWidget` reconciliation as the engine's source of
//! truth. Built alongside the existing `scene` module, not wired into
//! `Renderer` yet — see [`mount_legacy_widget`] for the compatibility
//! bridge between the two.

mod arena;
mod binding;
mod branch;
mod dirty;
mod events;
mod keyed;
mod mount;
mod mount_cx;
mod mutation;
mod node;
mod paint;
mod transaction;
mod view;

pub use binding::{create_binding, SharedRuntime};
pub use branch::create_branch;
pub use dirty::DirtyFlags;
pub use events::{HitEntry, PointerState};
pub use keyed::create_keyed_list;
pub use mount::mount_legacy_widget;
pub use mount_cx::MountCx;
pub use mutation::{Mutation, Transform2D};
pub use node::{
    Children, CustomNode, EventState, ImageNode, NodeKind, RuntimeNode, RuntimeNodeId, TextNode,
};
pub use paint::{
    BorderPrimitive, ImagePrimitive, PaintFragment, PaintOp, PaintPrimitive, PaintState,
    QuadPrimitive, RecordingPainter, TextPrimitive,
};
pub use transaction::RuntimeTransaction;
pub use view::{IntoView, View};

use arena::Arena;
use taffy::TaffyTree;

/// Intersects `rect` into `ambient` (or takes `rect` as-is if there's no
/// ambient clip yet). A non-overlapping intersection yields a zero-area
/// rect at `rect`'s origin rather than `None` — still clipped, just to
/// nothing.
fn clip_to(ambient: Option<crate::Rect>, rect: crate::Rect) -> crate::Rect {
    match ambient {
        Some(existing) => existing.intersect(rect).unwrap_or(crate::Rect {
            x: rect.x,
            y: rect.y,
            width: 0.0,
            height: 0.0,
        }),
        None => rect,
    }
}

pub struct Runtime {
    nodes: Arena<RuntimeNode>,
    root: Option<RuntimeNodeId>,
    taffy: TaffyTree<crate::MeasureFn>,
    /// Set when a mutation marks any node's LAYOUT/STRUCTURE dirty; lets
    /// [`Runtime::compute_layout`] skip `taffy` entirely otherwise.
    layout_dirty: bool,
    /// Bumped once per [`Runtime::compute_layout`] call.
    layout_epoch: u64,
    last_viewport: Option<crate::Size>,
    /// Nodes whose layout inputs changed since the last layout.
    layout_roots: Vec<RuntimeNodeId>,
    /// Makes the next rect sync visit every node, e.g. after the root moved.
    full_layout_sync: bool,
    /// Set when the hit-test list's membership or order may have changed
    /// (structure, interactivity, focusability, positioning); lets
    /// [`Runtime::rebuild_hit_test`] skip the tree walk otherwise.
    hit_test_dirty: bool,
    /// Listed nodes whose rect moved, patched in place when the list's
    /// membership is otherwise unchanged.
    hit_rects: Vec<RuntimeNodeId>,
    hit_entries: Vec<HitEntry>,
    focus_order: Vec<RuntimeNodeId>,
    pointer: PointerState,
    /// Nodes with a pending fragment regeneration — appended to whenever a
    /// mutation marks PAINT dirty, so [`Runtime::rebuild_paint`] only
    /// visits nodes that actually changed instead of walking the tree.
    paint_queue: Vec<RuntimeNodeId>,
    /// Set when a mutation marks any node's STRUCTURE dirty; lets
    /// [`Runtime::rebuild_paint`] skip re-deriving paint order otherwise.
    paint_order_dirty: bool,
    paint_order: Vec<RuntimeNodeId>,
    composite_queue: Vec<RuntimeNodeId>,
    /// Bumped once per [`Runtime::transaction`] call; see
    /// [`RuntimeNode::touched_stamp`](super::node::RuntimeNode::touched_stamp).
    transaction_stamp: u64,
}

impl Runtime {
    pub fn new() -> Self {
        Runtime {
            nodes: Arena::new(),
            root: None,
            taffy: TaffyTree::new(),
            layout_dirty: false,
            layout_epoch: 0,
            last_viewport: None,
            layout_roots: Vec::new(),
            full_layout_sync: false,
            hit_test_dirty: false,
            hit_rects: Vec::new(),
            hit_entries: Vec::new(),
            focus_order: Vec::new(),
            pointer: PointerState::default(),
            paint_queue: Vec::new(),
            paint_order_dirty: false,
            paint_order: Vec::new(),
            composite_queue: Vec::new(),
            transaction_stamp: 0,
        }
    }

    pub fn transaction(&mut self) -> RuntimeTransaction<'_> {
        self.transaction_stamp += 1;
        RuntimeTransaction::new(self, self.transaction_stamp)
    }

    pub fn get(&self, id: RuntimeNodeId) -> Option<&RuntimeNode> {
        self.nodes.get(id)
    }

    pub fn root(&self) -> Option<RuntimeNodeId> {
        self.root
    }

    pub fn set_root(&mut self, id: Option<RuntimeNodeId>) {
        if self.root != id {
            self.full_layout_sync = true;
            self.hit_test_dirty = true;
        }
        self.root = id;
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn is_layout_dirty(&self) -> bool {
        self.layout_dirty
    }

    pub fn layout_epoch(&self) -> u64 {
        self.layout_epoch
    }

    /// Recomputes layout only if something marked it dirty or the viewport
    /// itself changed, then marks `PAINT | HIT_TEST` on nodes whose
    /// window-space rect actually moved and returns their old+new rects as
    /// damage. No-op otherwise, or when there is no root.
    pub fn compute_layout(&mut self, viewport: crate::Size) -> Vec<crate::Rect> {
        let Some(root) = self.root else {
            return Vec::new();
        };
        let viewport_changed = self.last_viewport != Some(viewport);
        if !self.layout_dirty && !viewport_changed {
            return Vec::new();
        }
        self.last_viewport = Some(viewport);

        #[cfg(feature = "perf-metrics")]
        crate::metrics::record(|m| m.layout_runs += 1);
        #[cfg(feature = "perf-metrics")]
        let _span = tracing::info_span!("runtime_compute_layout").entered();

        let root_taffy = self
            .nodes
            .get(root)
            .expect("root exists in this runtime")
            .layout
            .taffy_node;
        self.taffy
            .compute_layout_with_measure(
                root_taffy,
                taffy::geometry::Size {
                    width: taffy::style::AvailableSpace::Definite(viewport.width),
                    height: taffy::style::AvailableSpace::Definite(viewport.height),
                },
                |known_dimensions, available_space, _node_id, measure, _style| match measure {
                    Some(measure) => {
                        #[cfg(feature = "perf-metrics")]
                        crate::metrics::record(|m| m.measure_calls += 1);
                        measure(known_dimensions, available_space)
                    }
                    None => taffy::geometry::Size::ZERO,
                },
            )
            .expect("layout computation should not fail for a well-formed tree");

        self.layout_dirty = false;
        self.layout_epoch += 1;
        self.mark_layout_paths();
        self.sync_layout_rects(root)
    }

    fn mark_layout_paths(&mut self) {
        for id in std::mem::take(&mut self.layout_roots) {
            let mut current = Some(id);
            while let Some(node) = current.and_then(|id| self.nodes.get_mut(id)) {
                if node.on_layout_path {
                    break;
                }
                node.on_layout_path = true;
                current = node.parent;
            }
        }
    }

    /// Iterative top-down pass (a deep tree can overflow the stack under
    /// recursion) computing each node's window-space rect from `taffy`'s
    /// parent-relative output. A subtree is skipped when it is not on a
    /// changed node's ancestor path and its root kept its rect: `taffy`
    /// lays it out from the same inputs, so nothing inside it moved.
    fn sync_layout_rects(&mut self, root: RuntimeNodeId) -> Vec<crate::Rect> {
        let full = std::mem::take(&mut self.full_layout_sync);
        let mut damage = Vec::new();
        let mut stack: Vec<(RuntimeNodeId, crate::Point)> = vec![(root, crate::Point::default())];
        while let Some((id, parent_origin)) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let layout = self
                .taffy
                .layout(node.layout.taffy_node)
                .expect("layout computed for every mounted node");
            let rect = crate::Rect {
                x: parent_origin.x + layout.location.x,
                y: parent_origin.y + layout.location.y,
                width: layout.size.width,
                height: layout.size.height,
            };
            let child_origin = crate::Point {
                x: rect.x,
                y: rect.y,
            };
            let children: Vec<RuntimeNodeId> = node.children.as_slice().to_vec();

            let node = self.nodes.get_mut(id).expect("checked above");
            let on_path = std::mem::take(&mut node.on_layout_path);
            let moved = node.layout.rect != rect;
            if moved {
                damage.push(node.layout.rect);
                damage.push(rect);
                node.layout.previous_rect = node.layout.rect;
                node.layout.rect = rect;
                node.layout.last_layout_epoch = self.layout_epoch;
                node.dirty |= DirtyFlags::PAINT | DirtyFlags::HIT_TEST;
                if node.hit_slot.is_some() {
                    self.hit_rects.push(id);
                }
                self.paint_queue.push(id);
                // A clipping node's rect feeds its children's effective_clip.
                // The node's own effective_clip is unaffected, so
                // rebuild_composite's own change-detection won't cascade to
                // them on its own — queue them here instead.
                if node.clips_children {
                    for &child in &children {
                        if let Some(child_node) = self.nodes.get_mut(child) {
                            child_node.dirty |= DirtyFlags::COMPOSITE;
                        }
                    }
                    self.composite_queue.extend(children.iter().copied());
                }
            }

            if full || on_path || moved {
                stack.extend(children.into_iter().map(|child| (child, child_origin)));
            }
        }
        damage
    }

    /// Rebuilds paint order if structure changed, then regenerates the
    /// fragment for every queued (PAINT-dirty) node — not a tree walk, so
    /// this scales with how much actually changed, not tree size. Returns
    /// each regenerated node's old and new bounds as damage.
    pub fn rebuild_paint(&mut self, colors: &creamui_theme::ColorScheme) -> Vec<crate::Rect> {
        if self.paint_order_dirty {
            self.rebuild_paint_order();
        }

        let queue = std::mem::take(&mut self.paint_queue);
        let mut damage = Vec::with_capacity(queue.len() * 2);
        for id in queue {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            if !node.dirty.contains(DirtyFlags::PAINT) {
                continue;
            }
            #[cfg(feature = "perf-metrics")]
            crate::metrics::record(|m| m.paint_nodes_recorded += 1);
            let old_bounds = node.paint.fragment.as_ref().map(|f| f.bounds);
            let fragment = paint::generate_fragment(node, colors);
            damage.extend(old_bounds);
            damage.push(fragment.bounds);
            let node = self.nodes.get_mut(id).expect("checked above");
            node.paint.fragment = Some(fragment);
            node.dirty.remove(DirtyFlags::PAINT);
        }
        damage
    }

    fn rebuild_paint_order(&mut self) {
        self.paint_order.clear();
        let Some(root) = self.root else {
            self.paint_order_dirty = false;
            return;
        };
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            self.paint_order.push(id);
            stack.extend(node.children.as_slice().iter().rev());
        }
        self.paint_order_dirty = false;
    }

    /// Every node in depth-first paint order, as of the last
    /// [`Runtime::rebuild_paint`].
    pub fn paint_order(&self) -> &[RuntimeNodeId] {
        &self.paint_order
    }

    /// Recomputes `effective_transform`/`effective_opacity`/`effective_clip`
    /// for every queued (COMPOSITE-dirty) node, cascading to a node's
    /// children whenever any of the three actually changed (they inherit
    /// all three) — no layout, no paint-fragment regeneration. Returns
    /// every node whose composited state was touched, so a renderer can
    /// reposition/re-blend/re-clip already-retained content for exactly
    /// those nodes.
    pub fn rebuild_composite(&mut self) -> Vec<RuntimeNodeId> {
        let mut queue = std::mem::take(&mut self.composite_queue);
        let mut touched = Vec::with_capacity(queue.len());
        let mut i = 0;
        while i < queue.len() {
            let id = queue[i];
            i += 1;

            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let (parent_transform, parent_opacity, parent_clip) = node
                .parent
                .and_then(|parent| self.nodes.get(parent))
                .map(|parent| {
                    let clip = if parent.clips_children {
                        Some(clip_to(parent.layout.effective_clip, parent.layout.rect))
                    } else {
                        parent.layout.effective_clip
                    };
                    (
                        parent.layout.effective_transform,
                        parent.layout.effective_opacity,
                        clip,
                    )
                })
                .unwrap_or((Transform2D::default(), 1.0, None));
            let own = node.transform;
            let new_transform = Transform2D {
                x: parent_transform.x + own.x,
                y: parent_transform.y + own.y,
            };
            let new_opacity = parent_opacity * node.opacity;

            let node = self.nodes.get_mut(id).expect("checked above");
            let changed = node.layout.effective_transform != new_transform
                || node.layout.effective_opacity != new_opacity
                || node.layout.effective_clip != parent_clip;
            node.layout.effective_transform = new_transform;
            node.layout.effective_opacity = new_opacity;
            node.layout.effective_clip = parent_clip;
            node.dirty.remove(DirtyFlags::COMPOSITE);
            #[cfg(feature = "perf-metrics")]
            crate::metrics::record(|m| m.composite_nodes_updated += 1);
            touched.push(id);

            if changed {
                queue.extend(node.children.as_slice().iter().copied());
            }
        }
        touched
    }

    /// Panics on the first broken invariant found: a child whose `parent`
    /// doesn't point back, a child claimed by more than one parent, a
    /// root set to an id this runtime doesn't hold, or a node whose
    /// `taffy` children don't match its runtime children. Intended for
    /// tests and debug assertions, not the hot path.
    pub fn check_invariants(&self) {
        use std::collections::HashSet;

        let mut claimed: HashSet<RuntimeNodeId> = HashSet::new();
        for (id, node) in self.nodes.iter() {
            for &child in node.children.as_slice() {
                let child_node = self
                    .nodes
                    .get(child)
                    .unwrap_or_else(|| panic!("{id:?} references missing child {child:?}"));
                assert_eq!(
                    child_node.parent,
                    Some(id),
                    "{child:?}'s parent does not point back to {id:?}"
                );
                assert!(
                    claimed.insert(child),
                    "{child:?} is claimed as a child by more than one parent"
                );
            }
            let taffy_children: Vec<RuntimeNodeId> = self
                .taffy
                .children(node.layout.taffy_node)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|taffy_child| {
                    self.nodes
                        .iter()
                        .find(|(_, n)| n.layout.taffy_node == taffy_child)
                        .map(|(id, _)| id)
                })
                .collect();
            assert_eq!(
                taffy_children,
                node.children.as_slice(),
                "{id:?}'s taffy children do not match its runtime children"
            );
        }
        if let Some(root) = self.root {
            assert!(self.nodes.contains(root), "root {root:?} does not exist");
        }
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized(width: f32, height: f32) -> taffy::style::Style {
        taffy::style::Style {
            size: taffy::geometry::Size {
                width: taffy::style::Dimension::Length(width),
                height: taffy::style::Dimension::Length(height),
            },
            ..Default::default()
        }
    }

    const VIEWPORT: crate::Size = crate::Size {
        width: 800.0,
        height: 600.0,
    };

    #[test]
    fn incremental_rect_sync_matches_a_full_one() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let root = tx.create_node(NodeKind::Container);
        tx.apply(Mutation::SetLayoutStyle {
            node: root,
            style: taffy::style::Style {
                flex_direction: taffy::style::FlexDirection::Column,
                ..Default::default()
            },
        });
        let mut leaves = Vec::new();
        let mut nested = Vec::new();
        for _ in 0..4 {
            let row = tx.create_node(NodeKind::Container);
            tx.insert_child(root, row, None);
            let leaf = tx.create_node(NodeKind::Container);
            tx.apply(Mutation::SetLayoutStyle {
                node: leaf,
                style: sized(20.0, 20.0),
            });
            tx.insert_child(row, leaf, None);
            let inner = tx.create_node(NodeKind::Container);
            tx.apply(Mutation::SetLayoutStyle {
                node: inner,
                style: sized(10.0, 10.0),
            });
            tx.insert_child(row, inner, None);
            leaves.push(leaf);
            nested.push(inner);
        }
        drop(tx);
        runtime.set_root(Some(root));
        runtime.compute_layout(VIEWPORT);
        let before = runtime.get(nested[3]).unwrap().layout.rect;

        let mut tx = runtime.transaction();
        tx.apply(Mutation::SetLayoutStyle {
            node: leaves[0],
            style: sized(20.0, 50.0),
        });
        drop(tx);
        runtime.compute_layout(VIEWPORT);
        assert_eq!(
            runtime.get(nested[3]).unwrap().layout.rect.y,
            before.y + 30.0
        );

        runtime.full_layout_sync = true;
        runtime.layout_dirty = true;
        assert!(runtime.compute_layout(VIEWPORT).is_empty());
    }

    #[test]
    fn create_and_insert_child_link_both_directions() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let parent = tx.create_node(NodeKind::Container);
        let child = tx.create_node(NodeKind::Container);
        tx.insert_child(parent, child, None);

        assert_eq!(runtime.get(parent).unwrap().children.as_slice(), &[child]);
        assert_eq!(runtime.get(child).unwrap().parent, Some(parent));
        runtime.check_invariants();
    }

    #[test]
    fn remove_subtree_drops_descendants_and_unlinks_from_parent() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let parent = tx.create_node(NodeKind::Container);
        let child = tx.create_node(NodeKind::Container);
        let grandchild = tx.create_node(NodeKind::Container);
        tx.insert_child(parent, child, None);
        tx.insert_child(child, grandchild, None);

        tx.remove_subtree(child);

        assert!(runtime.get(child).is_none());
        assert!(runtime.get(grandchild).is_none());
        assert!(runtime.get(parent).unwrap().children.is_empty());
        runtime.check_invariants();
    }

    #[test]
    fn touched_reports_every_node_a_transaction_affected() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let parent = tx.create_node(NodeKind::Container);
        let child = tx.create_node(NodeKind::Container);
        tx.insert_child(parent, child, None);

        // `insert_child` touches `parent` again, but it's already in
        // `touched` from its own `create_node` — deduplicated, not counted
        // twice.
        assert_eq!(tx.touched().len(), 2);
    }

    #[test]
    fn touching_the_same_existing_node_many_times_in_one_transaction_is_reported_once() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let node = tx.create_node(NodeKind::Container);
        drop(tx);

        let mut tx = runtime.transaction();
        tx.apply(Mutation::SetTransform {
            node,
            transform: Transform2D { x: 1.0, y: 0.0 },
        });
        tx.apply(Mutation::SetTransform {
            node,
            transform: Transform2D { x: 2.0, y: 0.0 },
        });
        assert_eq!(tx.touched(), &[node]);
    }

    #[test]
    fn a_node_touched_in_an_earlier_transaction_is_still_reported_in_a_later_one() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let node = tx.create_node(NodeKind::Container);
        drop(tx);

        let mut tx = runtime.transaction();
        tx.apply(Mutation::SetTransform {
            node,
            transform: Transform2D { x: 1.0, y: 0.0 },
        });
        assert_eq!(tx.touched(), &[node]);
    }

    #[test]
    fn set_layout_style_marks_layout_dirty_only_when_it_actually_changes() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let node = tx.create_node(NodeKind::Container);
        drop(tx);

        let unchanged_style = runtime.get(node).unwrap().layout_style.clone();
        let mut tx = runtime.transaction();
        tx.apply(Mutation::SetLayoutStyle {
            node,
            style: unchanged_style,
        });
        assert!(
            tx.touched().is_empty(),
            "setting the same layout style must not mark anything dirty"
        );

        let mut different_style = taffy::style::Style::default();
        different_style.flex_grow = 1.0;
        tx.apply(Mutation::SetLayoutStyle {
            node,
            style: different_style,
        });
        assert_eq!(tx.touched(), &[node]);
        drop(tx);
        assert!(runtime
            .get(node)
            .unwrap()
            .dirty
            .contains(DirtyFlags::LAYOUT));
    }

    fn size(width: f32, height: f32) -> crate::Size {
        crate::Size { width, height }
    }

    fn full_size_root(runtime: &mut Runtime) -> RuntimeNodeId {
        let mut tx = runtime.transaction();
        let root = tx.create_node(NodeKind::Container);
        tx.apply(Mutation::SetLayoutStyle {
            node: root,
            style: taffy::style::Style {
                size: taffy::geometry::Size {
                    width: taffy::style::Dimension::Percent(1.0),
                    height: taffy::style::Dimension::Percent(1.0),
                },
                ..Default::default()
            },
        });
        root
    }

    #[test]
    fn compute_layout_is_a_noop_with_no_root() {
        let mut runtime = Runtime::new();
        assert!(runtime.compute_layout(size(100.0, 100.0)).is_empty());
    }

    #[test]
    fn creating_the_root_produces_damage_on_first_compute() {
        let mut runtime = Runtime::new();
        let root = full_size_root(&mut runtime);
        runtime.set_root(Some(root));
        assert!(runtime.is_layout_dirty());

        let damage = runtime.compute_layout(size(100.0, 100.0));
        assert!(!damage.is_empty());
        assert!(!runtime.is_layout_dirty());
        assert_eq!(runtime.get(root).unwrap().layout.rect.width, 100.0);
        assert_eq!(runtime.layout_epoch(), 1);
    }

    #[test]
    fn a_second_compute_with_nothing_changed_produces_no_damage() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        runtime.compute_layout(size(100.0, 100.0));

        assert!(runtime.compute_layout(size(100.0, 100.0)).is_empty());
        assert_eq!(runtime.layout_epoch(), 1);
    }

    #[test]
    fn resizing_the_viewport_recomputes_even_with_nothing_else_dirty() {
        let mut runtime = Runtime::new();
        let root = full_size_root(&mut runtime);
        runtime.set_root(Some(root));
        runtime.compute_layout(size(100.0, 100.0));

        let damage = runtime.compute_layout(size(200.0, 100.0));
        assert!(!damage.is_empty());
        assert_eq!(runtime.get(root).unwrap().layout.rect.width, 200.0);
    }

    #[test]
    fn paint_only_changes_never_mark_layout_dirty() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        runtime.compute_layout(size(100.0, 100.0));

        let mut tx = runtime.transaction();
        tx.apply(Mutation::SetPaintStyle {
            node: root,
            style: crate::PaintStyle {
                background: Some(creamui_theme::Color::rgb(1, 2, 3).into()),
                ..Default::default()
            },
        });
        drop(tx);

        assert!(!runtime.is_layout_dirty());
        assert!(runtime.compute_layout(size(100.0, 100.0)).is_empty());
    }

    #[test]
    fn setting_the_same_measure_fingerprint_twice_skips_the_second_write() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let node = tx.create_node(NodeKind::Container);
        tx.apply(Mutation::SetMeasure {
            node,
            measure: Some(Box::new(|_, _| taffy::geometry::Size::ZERO)),
            fingerprint: Some(7),
        });
        drop(tx);
        let touched_after_first = runtime.get(node).unwrap().layout.measure_fingerprint;
        assert_eq!(touched_after_first, Some(7));

        let mut tx = runtime.transaction();
        tx.apply(Mutation::SetMeasure {
            node,
            measure: Some(Box::new(|_, _| taffy::geometry::Size::ZERO)),
            fingerprint: Some(7),
        });
        assert!(
            tx.touched().is_empty(),
            "an unchanged measure fingerprint must not touch the node"
        );
    }

    fn background_node(runtime: &mut Runtime, color: creamui_theme::Color) -> RuntimeNodeId {
        let mut tx = runtime.transaction();
        let node = tx.create_node(NodeKind::Container);
        tx.apply(Mutation::SetPaintStyle {
            node,
            style: crate::PaintStyle {
                background: Some(color.into()),
                ..Default::default()
            },
        });
        node
    }

    fn text_node(
        runtime: &mut Runtime,
        text: &str,
        style: crate::TypographyStyle,
    ) -> RuntimeNodeId {
        let mut tx = runtime.transaction();
        let node = tx.create_node(NodeKind::Text(TextNode { text: text.into() }));
        tx.apply(Mutation::SetTypographyStyle { node, style });
        node
    }

    #[test]
    fn rebuild_paint_generates_a_quad_for_a_background() {
        let mut runtime = Runtime::new();
        let node = background_node(&mut runtime, creamui_theme::Color::rgb(1, 2, 3));

        runtime.rebuild_paint(&creamui_theme::ColorScheme::default());

        let fragment = runtime.get(node).unwrap().paint.fragment.as_ref().unwrap();
        assert_eq!(
            fragment.ops,
            vec![paint::PaintOp::Primitive(paint::PaintPrimitive::Quad(
                paint::QuadPrimitive {
                    rect: crate::Rect::default(),
                    color: creamui_theme::Color::rgb(1, 2, 3),
                    corner_radius: 0.0,
                }
            ))]
        );
    }

    #[test]
    fn rebuild_paint_carries_underline_and_strikethrough_into_the_text_primitive() {
        let mut runtime = Runtime::new();
        let node = text_node(
            &mut runtime,
            "hi",
            crate::TypographyStyle {
                underline: Some(true),
                strikethrough: Some(true),
                ..Default::default()
            },
        );

        runtime.rebuild_paint(&creamui_theme::ColorScheme::default());

        let fragment = runtime.get(node).unwrap().paint.fragment.as_ref().unwrap();
        let paint::PaintOp::Primitive(paint::PaintPrimitive::Text(text)) = &fragment.ops[0] else {
            panic!("expected a text primitive");
        };
        assert!(text.underline);
        assert!(text.strikethrough);
    }

    #[test]
    fn rebuild_paint_only_regenerates_the_node_that_actually_changed() {
        let mut runtime = Runtime::new();
        let a = background_node(&mut runtime, creamui_theme::Color::rgb(1, 1, 1));
        let _b = background_node(&mut runtime, creamui_theme::Color::rgb(2, 2, 2));
        let colors = creamui_theme::ColorScheme::default();
        runtime.rebuild_paint(&colors);

        let mut tx = runtime.transaction();
        tx.apply(Mutation::SetPaintStyle {
            node: a,
            style: crate::PaintStyle {
                background: Some(creamui_theme::Color::rgb(9, 9, 9).into()),
                ..Default::default()
            },
        });
        drop(tx);

        let damage = runtime.rebuild_paint(&colors);
        assert_eq!(
            damage.len(),
            2,
            "one regenerated fragment: old + new bounds"
        );
    }

    #[test]
    fn rebuild_paint_is_a_noop_when_nothing_is_dirty() {
        let mut runtime = Runtime::new();
        background_node(&mut runtime, creamui_theme::Color::rgb(1, 1, 1));
        let colors = creamui_theme::ColorScheme::default();
        runtime.rebuild_paint(&colors);

        assert!(runtime.rebuild_paint(&colors).is_empty());
    }

    #[test]
    fn paint_order_lists_nodes_depth_first() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        let child = {
            let mut tx = runtime.transaction();
            let child = tx.create_node(NodeKind::Container);
            tx.insert_child(root, child, None);
            child
        };

        runtime.rebuild_paint(&creamui_theme::ColorScheme::default());
        assert_eq!(runtime.paint_order(), &[root, child]);
    }

    #[test]
    fn rebuild_composite_sets_a_leafs_effective_transform_to_its_own() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let node = tx.create_node(NodeKind::Container);
        tx.apply(Mutation::SetTransform {
            node,
            transform: Transform2D { x: 5.0, y: 7.0 },
        });
        drop(tx);

        runtime.rebuild_composite();
        assert_eq!(
            runtime.get(node).unwrap().layout.effective_transform,
            Transform2D { x: 5.0, y: 7.0 }
        );
    }

    #[test]
    fn rebuild_composite_cascades_a_parents_transform_to_its_children() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let parent = tx.create_node(NodeKind::Container);
        let child = tx.create_node(NodeKind::Container);
        tx.insert_child(parent, child, None);
        tx.apply(Mutation::SetTransform {
            node: parent,
            transform: Transform2D { x: 10.0, y: 0.0 },
        });
        drop(tx);

        runtime.rebuild_composite();
        assert_eq!(
            runtime.get(child).unwrap().layout.effective_transform,
            Transform2D { x: 10.0, y: 0.0 }
        );
    }

    #[test]
    fn rebuild_composite_does_not_touch_the_paint_fragment() {
        let mut runtime = Runtime::new();
        let node = background_node(&mut runtime, creamui_theme::Color::rgb(1, 2, 3));
        runtime.rebuild_paint(&creamui_theme::ColorScheme::default());
        let fragment_before = runtime
            .get(node)
            .unwrap()
            .paint
            .fragment
            .as_ref()
            .unwrap()
            .bounds;

        let mut tx = runtime.transaction();
        tx.apply(Mutation::SetTransform {
            node,
            transform: Transform2D { x: 3.0, y: 4.0 },
        });
        drop(tx);

        assert!(runtime
            .rebuild_paint(&creamui_theme::ColorScheme::default())
            .is_empty());
        runtime.rebuild_composite();
        let fragment_after = runtime
            .get(node)
            .unwrap()
            .paint
            .fragment
            .as_ref()
            .unwrap()
            .bounds;
        assert_eq!(fragment_before, fragment_after);
    }

    #[test]
    fn rebuild_composite_is_a_noop_when_nothing_is_dirty() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let node = tx.create_node(NodeKind::Container);
        tx.apply(Mutation::SetTransform {
            node,
            transform: Transform2D { x: 1.0, y: 1.0 },
        });
        drop(tx);
        runtime.rebuild_composite();

        assert!(runtime.rebuild_composite().is_empty());
    }

    #[test]
    fn rebuild_composite_sets_a_leafs_effective_opacity_to_its_own() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let node = tx.create_node(NodeKind::Container);
        tx.apply(Mutation::SetOpacity { node, opacity: 0.4 });
        drop(tx);

        runtime.rebuild_composite();
        assert_eq!(runtime.get(node).unwrap().layout.effective_opacity, 0.4);
    }

    #[test]
    fn rebuild_composite_multiplies_a_parents_opacity_into_its_children() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let parent = tx.create_node(NodeKind::Container);
        let child = tx.create_node(NodeKind::Container);
        tx.insert_child(parent, child, None);
        tx.apply(Mutation::SetOpacity {
            node: parent,
            opacity: 0.5,
        });
        tx.apply(Mutation::SetOpacity {
            node: child,
            opacity: 0.5,
        });
        drop(tx);

        runtime.rebuild_composite();
        assert_eq!(runtime.get(parent).unwrap().layout.effective_opacity, 0.5);
        assert_eq!(runtime.get(child).unwrap().layout.effective_opacity, 0.25);
    }

    #[test]
    fn set_opacity_clamps_to_the_zero_one_range() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let over = tx.create_node(NodeKind::Container);
        let under = tx.create_node(NodeKind::Container);
        tx.apply(Mutation::SetOpacity {
            node: over,
            opacity: 2.0,
        });
        tx.apply(Mutation::SetOpacity {
            node: under,
            opacity: -1.0,
        });
        drop(tx);

        assert_eq!(runtime.get(over).unwrap().opacity, 1.0);
        assert_eq!(runtime.get(under).unwrap().opacity, 0.0);
    }

    #[test]
    fn a_default_nodes_effective_opacity_is_fully_opaque() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let node = tx.create_node(NodeKind::Container);
        drop(tx);

        assert_eq!(runtime.get(node).unwrap().layout.effective_opacity, 1.0);
    }

    fn set_rect(runtime: &mut Runtime, id: RuntimeNodeId, rect: crate::Rect) {
        runtime.nodes.get_mut(id).unwrap().layout.rect = rect;
    }

    fn rect(x: f32, y: f32, width: f32, height: f32) -> crate::Rect {
        crate::Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn rebuild_composite_sets_a_childs_effective_clip_from_its_clipping_parent() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let parent = tx.create_node(NodeKind::Container);
        let child = tx.create_node(NodeKind::Container);
        tx.insert_child(parent, child, None);
        tx.apply(Mutation::SetClipsChildren {
            node: parent,
            clips_children: true,
        });
        drop(tx);
        set_rect(&mut runtime, parent, rect(0.0, 0.0, 100.0, 50.0));

        runtime.rebuild_composite();

        assert_eq!(
            runtime.get(child).unwrap().layout.effective_clip,
            Some(rect(0.0, 0.0, 100.0, 50.0))
        );
        assert_eq!(runtime.get(parent).unwrap().layout.effective_clip, None);
    }

    #[test]
    fn rebuild_composite_intersects_nested_clips() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let grandparent = tx.create_node(NodeKind::Container);
        let parent = tx.create_node(NodeKind::Container);
        let child = tx.create_node(NodeKind::Container);
        tx.insert_child(grandparent, parent, None);
        tx.insert_child(parent, child, None);
        tx.apply(Mutation::SetClipsChildren {
            node: grandparent,
            clips_children: true,
        });
        tx.apply(Mutation::SetClipsChildren {
            node: parent,
            clips_children: true,
        });
        drop(tx);
        set_rect(&mut runtime, grandparent, rect(0.0, 0.0, 100.0, 100.0));
        set_rect(&mut runtime, parent, rect(50.0, 50.0, 100.0, 100.0));

        runtime.rebuild_composite();

        assert_eq!(
            runtime.get(child).unwrap().layout.effective_clip,
            Some(rect(50.0, 50.0, 50.0, 50.0))
        );
    }

    #[test]
    fn a_clip_with_no_overlap_becomes_a_zero_area_rect_not_none() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let grandparent = tx.create_node(NodeKind::Container);
        let parent = tx.create_node(NodeKind::Container);
        let child = tx.create_node(NodeKind::Container);
        tx.insert_child(grandparent, parent, None);
        tx.insert_child(parent, child, None);
        tx.apply(Mutation::SetClipsChildren {
            node: grandparent,
            clips_children: true,
        });
        tx.apply(Mutation::SetClipsChildren {
            node: parent,
            clips_children: true,
        });
        drop(tx);
        set_rect(&mut runtime, grandparent, rect(0.0, 0.0, 10.0, 10.0));
        set_rect(&mut runtime, parent, rect(500.0, 500.0, 10.0, 10.0));

        runtime.rebuild_composite();

        let clip = runtime.get(child).unwrap().layout.effective_clip;
        assert_eq!(
            clip,
            Some(rect(500.0, 500.0, 0.0, 0.0)),
            "fully clipped away must stay Some(..) with zero area, not None"
        );
    }

    #[test]
    fn a_non_clipping_node_passes_the_ambient_clip_through_unchanged() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let grandparent = tx.create_node(NodeKind::Container);
        let parent = tx.create_node(NodeKind::Container);
        let child = tx.create_node(NodeKind::Container);
        tx.insert_child(grandparent, parent, None);
        tx.insert_child(parent, child, None);
        tx.apply(Mutation::SetClipsChildren {
            node: grandparent,
            clips_children: true,
        });
        drop(tx);
        set_rect(&mut runtime, grandparent, rect(0.0, 0.0, 100.0, 100.0));
        set_rect(&mut runtime, parent, rect(500.0, 500.0, 10.0, 10.0));

        runtime.rebuild_composite();

        assert_eq!(
            runtime.get(child).unwrap().layout.effective_clip,
            Some(rect(0.0, 0.0, 100.0, 100.0))
        );
    }

    #[test]
    fn toggling_clips_children_updates_the_childrens_effective_clip_next_pass() {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let parent = tx.create_node(NodeKind::Container);
        let child = tx.create_node(NodeKind::Container);
        tx.insert_child(parent, child, None);
        drop(tx);
        set_rect(&mut runtime, parent, rect(0.0, 0.0, 20.0, 20.0));
        runtime.rebuild_composite();
        assert_eq!(runtime.get(child).unwrap().layout.effective_clip, None);

        let mut tx = runtime.transaction();
        tx.apply(Mutation::SetClipsChildren {
            node: parent,
            clips_children: true,
        });
        drop(tx);
        runtime.rebuild_composite();

        assert_eq!(
            runtime.get(child).unwrap().layout.effective_clip,
            Some(rect(0.0, 0.0, 20.0, 20.0))
        );
    }

    #[test]
    fn changing_a_clipping_nodes_layout_rect_updates_its_childrens_effective_clip() {
        let mut runtime = Runtime::new();
        let root = full_size_root(&mut runtime);
        runtime.set_root(Some(root));
        let child = {
            let mut tx = runtime.transaction();
            let child = tx.create_node(NodeKind::Container);
            tx.insert_child(root, child, None);
            tx.apply(Mutation::SetClipsChildren {
                node: root,
                clips_children: true,
            });
            child
        };
        runtime.compute_layout(size(100.0, 100.0));
        runtime.rebuild_composite();
        assert_eq!(
            runtime.get(child).unwrap().layout.effective_clip,
            Some(rect(0.0, 0.0, 100.0, 100.0))
        );

        runtime.compute_layout(size(200.0, 150.0));
        runtime.rebuild_composite();

        assert_eq!(
            runtime.get(child).unwrap().layout.effective_clip,
            Some(rect(0.0, 0.0, 200.0, 150.0)),
            "a clipping ancestor's own rect change must reach its children's \
             effective_clip on the next rebuild_composite, with no explicit \
             SetClipsChildren/SetTransform/SetOpacity mutation in between"
        );
    }

    /// A tiny xorshift generator, so this test is reproducible without a
    /// `rand` dependency.
    struct Xorshift(u64);
    impl Xorshift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, bound: usize) -> usize {
            (self.next() % bound as u64) as usize
        }
    }

    fn is_ancestor(runtime: &Runtime, candidate: RuntimeNodeId, of: RuntimeNodeId) -> bool {
        let mut current = runtime.get(of).and_then(|n| n.parent);
        while let Some(id) = current {
            if id == candidate {
                return true;
            }
            current = runtime.get(id).and_then(|n| n.parent);
        }
        false
    }

    #[test]
    fn random_create_insert_remove_sequences_never_break_invariants() {
        let mut rng = Xorshift(0x9E3779B97F4A7C15);
        let mut runtime = Runtime::new();
        let mut live: Vec<RuntimeNodeId> = Vec::new();

        for _ in 0..500 {
            // A caller must never make a node a descendant of itself; the
            // ancestry check needs `runtime` before `tx` borrows it (see
            // `insert_child`'s doc comment).
            let insert_pair = if live.len() >= 2 {
                let parent = live[rng.below(live.len())];
                let child = live[rng.below(live.len())];
                (parent != child && !is_ancestor(&runtime, child, parent))
                    .then_some((parent, child))
            } else {
                None
            };

            let mut tx = runtime.transaction();
            match rng.below(3) {
                0 => {
                    let id = tx.create_node(NodeKind::Container);
                    live.push(id);
                }
                1 => {
                    if let Some((parent, child)) = insert_pair {
                        tx.insert_child(parent, child, None);
                    }
                }
                2 if !live.is_empty() => {
                    let index = rng.below(live.len());
                    tx.remove_subtree(live[index]);
                }
                _ => {}
            }
            drop(tx);
            live.retain(|&id| runtime.get(id).is_some());
            runtime.check_invariants();
        }
    }
}
