use super::dirty::DirtyFlags;
use super::node::RuntimeNodeId;
use super::Runtime;
use std::collections::HashMap;

const HIT_CELL_SIZE: f32 = 128.0;
const MAX_INDEXED_CELLS: i64 = 64;

fn hit_cell(value: f32) -> i32 {
    (value / HIT_CELL_SIZE).floor() as i32
}

fn cell_range(rect: crate::Rect) -> Option<(i32, i32, i32, i32)> {
    let x1 = rect.x + rect.width;
    let y1 = rect.y + rect.height;
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !x1.is_finite()
        || !y1.is_finite()
        || rect.width < 0.0
        || rect.height < 0.0
    {
        return None;
    }
    let (x0, x1, y0, y1) = (
        hit_cell(rect.x),
        hit_cell(x1),
        hit_cell(rect.y),
        hit_cell(y1),
    );
    let width = i64::from(x1) - i64::from(x0) + 1;
    let height = i64::from(y1) - i64::from(y0) + 1;
    if width > MAX_INDEXED_CELLS || height > MAX_INDEXED_CELLS || width * height > MAX_INDEXED_CELLS
    {
        return None;
    }
    Some((x0, x1, y0, y1))
}

fn visible_hit_rect(node: &super::node::RuntimeNode) -> crate::Rect {
    let rect = crate::Rect {
        x: node.layout.rect.x + node.layout.effective_transform.x,
        y: node.layout.rect.y + node.layout.effective_transform.y,
        ..node.layout.rect
    };
    node.layout
        .effective_clip
        .map(|clip| {
            rect.intersect(clip).unwrap_or(crate::Rect {
                width: 0.0,
                height: 0.0,
                ..rect
            })
        })
        .unwrap_or(rect)
}

#[derive(Default)]
pub(super) struct HitIndex {
    cells: HashMap<(i32, i32), Vec<u32>>,
    spanning: Vec<u32>,
}

impl HitIndex {
    fn insert(&mut self, slot: u32, rect: crate::Rect) {
        if let Some((x0, x1, y0, y1)) = cell_range(rect) {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    self.cells.entry((x, y)).or_default().push(slot);
                }
            }
        } else {
            self.spanning.push(slot);
        }
    }

    fn remove(&mut self, slot: u32, rect: crate::Rect) {
        if let Some((x0, x1, y0, y1)) = cell_range(rect) {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if let std::collections::hash_map::Entry::Occupied(mut entry) =
                        self.cells.entry((x, y))
                    {
                        entry.get_mut().retain(|&candidate| candidate != slot);
                        if entry.get().is_empty() {
                            entry.remove();
                        }
                    }
                }
            }
        } else {
            self.spanning.retain(|&candidate| candidate != slot);
        }
    }

    fn hit(
        &self,
        point: crate::Point,
        entries: &[HitEntry],
        ranks: &[usize],
    ) -> Option<RuntimeNodeId> {
        let local = self
            .cells
            .get(&(hit_cell(point.x), hit_cell(point.y)))
            .and_then(|slots| {
                slots
                    .iter()
                    .copied()
                    .filter(|&slot| {
                        let rect = entries[slot as usize].rect;
                        rect.width > 0.0 && rect.height > 0.0 && rect.contains(point)
                    })
                    .max_by_key(|&slot| ranks[slot as usize])
            });
        let spanning = self
            .spanning
            .iter()
            .copied()
            .filter(|&slot| {
                let rect = entries[slot as usize].rect;
                rect.width > 0.0 && rect.height > 0.0 && rect.contains(point)
            })
            .max_by_key(|&slot| ranks[slot as usize]);
        local
            .into_iter()
            .chain(spanning)
            .max_by_key(|&slot| ranks[slot as usize])
            .map(|slot| entries[slot as usize].node)
    }
}

#[derive(Clone, Copy)]
pub struct HitEntry {
    pub node: RuntimeNodeId,
    pub rect: crate::Rect,
}

#[derive(Default)]
pub struct PointerState {
    pub hovered: Option<RuntimeNodeId>,
    pub pressed: Option<RuntimeNodeId>,
    pub focused: Option<RuntimeNodeId>,
    pub pointer_capture: Option<RuntimeNodeId>,
}

impl Runtime {
    fn insert_hit_entry(&mut self, id: RuntimeNodeId, rank: usize) {
        let Some(node) = self.nodes.get(id) else {
            return;
        };
        let rect = visible_hit_rect(node);
        let slot = self.hit_entries.len() as u32;
        self.hit_entries.push(HitEntry { node: id, rect });
        self.hit_ranks.push(rank);
        self.hit_index.insert(slot, rect);
        self.nodes.get_mut(id).expect("node exists").hit_slot = Some(slot);
        #[cfg(feature = "perf-metrics")]
        crate::metrics::record(|m| m.hit_nodes_updated += 1);
    }

    fn remove_hit_entry(&mut self, slot: u32) {
        let index = slot as usize;
        let last = self.hit_entries.len() - 1;
        let removed = self.hit_entries.swap_remove(index);
        self.hit_ranks.swap_remove(index);
        self.hit_index.remove(slot, removed.rect);
        #[cfg(feature = "perf-metrics")]
        crate::metrics::record(|m| m.hit_nodes_updated += 1);
        if let Some(node) = self.nodes.get_mut(removed.node) {
            node.hit_slot = None;
        }
        if index != last {
            let moved = self.hit_entries[index];
            self.hit_index.remove(last as u32, moved.rect);
            self.hit_index.insert(slot, moved.rect);
            if let Some(node) = self.nodes.get_mut(moved.node) {
                node.hit_slot = Some(slot);
            }
        }
    }

    fn sync_hit_rects(&mut self) {
        for id in self.hit_rects.drain(..) {
            if let Some(node) = self.nodes.get(id) {
                if let Some(slot) = node.hit_slot {
                    let entry = &mut self.hit_entries[slot as usize];
                    let rect = visible_hit_rect(node);
                    if entry.rect != rect {
                        self.hit_index.remove(slot, entry.rect);
                        entry.rect = rect;
                        self.hit_index.insert(slot, rect);
                        #[cfg(feature = "perf-metrics")]
                        crate::metrics::record(|m| m.hit_nodes_updated += 1);
                    }
                }
            }
        }
    }

    fn rebuild_focus_order(&mut self) {
        self.focus_order.clear();
        let Some(root) = self.root else {
            return;
        };
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            if node.events.focusable {
                self.focus_order.push(id);
            }
            stack.extend(node.children.as_slice().iter().rev().copied());
        }
    }

    /// Updates hit membership, paint ranks, focus order, and moved hit regions.
    pub fn rebuild_hit_test(&mut self) {
        if self.hit_test_dirty {
            if self.hit_order_dirty {
                if self.paint_order_dirty {
                    self.rebuild_paint_order();
                }
                self.paint_ranks.clear();
                self.paint_ranks.extend(
                    self.paint_order
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(rank, id)| (id, rank)),
                );
                let mut slot = 0;
                while slot < self.hit_entries.len() {
                    let id = self.hit_entries[slot].node;
                    let rank = self.paint_ranks.get(&id).copied().filter(|_| {
                        self.nodes
                            .get(id)
                            .is_some_and(|node| node.events.is_interactive())
                    });
                    if let Some(rank) = rank {
                        #[cfg(feature = "perf-metrics")]
                        if self.hit_ranks[slot] != rank {
                            crate::metrics::record(|m| m.hit_nodes_updated += 1);
                        }
                        self.hit_ranks[slot] = rank;
                        slot += 1;
                    } else {
                        self.remove_hit_entry(slot as u32);
                    }
                }
                for index in 0..self.paint_order.len() {
                    let id = self.paint_order[index];
                    if self
                        .nodes
                        .get(id)
                        .is_some_and(|node| node.events.is_interactive() && node.hit_slot.is_none())
                    {
                        self.insert_hit_entry(id, index);
                    }
                }
            } else {
                for id in std::mem::take(&mut self.hit_membership) {
                    let rank = self.paint_ranks.get(&id).copied().filter(|_| {
                        self.nodes
                            .get(id)
                            .is_some_and(|node| node.events.is_interactive())
                    });
                    let slot = self.nodes.get(id).and_then(|node| node.hit_slot);
                    match (slot, rank) {
                        (Some(slot), None) => self.remove_hit_entry(slot),
                        (None, Some(rank)) => self.insert_hit_entry(id, rank),
                        _ => {}
                    }
                }
            }
            if self.focus_order_dirty {
                self.rebuild_focus_order();
            }
            self.hit_membership.clear();
            self.hit_order_dirty = false;
            self.focus_order_dirty = false;
            self.hit_test_dirty = false;
        }
        self.sync_hit_rects();
    }

    /// Topmost interactive node containing `point`, from the retained hit
    /// index built by the last [`Runtime::rebuild_hit_test`].
    pub fn hit_test(&self, point: crate::Point) -> Option<RuntimeNodeId> {
        self.hit_index
            .hit(point, &self.hit_entries, &self.hit_ranks)
    }

    /// [`PointerState::pointer_capture`] if set, otherwise [`Runtime::hit_test`].
    pub fn route_pointer(&self, point: crate::Point) -> Option<RuntimeNodeId> {
        self.pointer
            .pointer_capture
            .or_else(|| self.hit_test(point))
    }

    pub fn capture_pointer(&mut self, node: RuntimeNodeId) {
        self.pointer.pointer_capture = Some(node);
    }

    pub fn release_pointer_capture(&mut self) {
        self.pointer.pointer_capture = None;
    }

    pub fn pointer(&self) -> &PointerState {
        &self.pointer
    }

    fn mark_interaction_dirty(&mut self, node: RuntimeNodeId) {
        self.record_invalidation(node, DirtyFlags::PAINT);
        if let Some(n) = self.nodes.get_mut(node) {
            if !n.dirty.contains(DirtyFlags::PAINT) {
                n.dirty |= DirtyFlags::PAINT;
                self.paint_queue.push(node);
            }
        }
    }

    /// Updates the hovered node, invalidating paint on the old and new
    /// target. Returns `false` (marking nothing) when `node` is already
    /// hovered.
    pub fn set_hovered(&mut self, node: Option<RuntimeNodeId>) -> bool {
        if self.pointer.hovered == node {
            return false;
        }
        if let Some(old) = self.pointer.hovered {
            self.mark_interaction_dirty(old);
        }
        if let Some(new) = node {
            self.mark_interaction_dirty(new);
        }
        self.pointer.hovered = node;
        true
    }

    pub fn set_pressed(&mut self, node: Option<RuntimeNodeId>) -> bool {
        if self.pointer.pressed == node {
            return false;
        }
        if let Some(old) = self.pointer.pressed {
            self.mark_interaction_dirty(old);
        }
        if let Some(new) = node {
            self.mark_interaction_dirty(new);
        }
        self.pointer.pressed = node;
        true
    }

    pub fn set_focused(&mut self, node: Option<RuntimeNodeId>) -> bool {
        if self.pointer.focused == node {
            return false;
        }
        if let Some(old) = self.pointer.focused {
            self.mark_interaction_dirty(old);
        }
        if let Some(new) = node {
            self.mark_interaction_dirty(new);
        }
        self.pointer.focused = node;
        true
    }

    /// The next (or, if `backwards`, previous) focusable node after
    /// `current` in the retained focus order, wrapping around. `None` when
    /// nothing is focusable.
    pub fn next_focus(
        &self,
        current: Option<RuntimeNodeId>,
        backwards: bool,
    ) -> Option<RuntimeNodeId> {
        if self.focus_order.is_empty() {
            return None;
        }
        let index = current.and_then(|id| self.focus_order.iter().position(|&n| n == id));
        let next_index = match (index, backwards) {
            (Some(i), true) => (i + self.focus_order.len() - 1) % self.focus_order.len(),
            (Some(i), false) => (i + 1) % self.focus_order.len(),
            (None, true) => self.focus_order.len() - 1,
            (None, false) => 0,
        };
        Some(self.focus_order[next_index])
    }

    /// Walks from the node at `point` up through its ancestors to the
    /// nearest one with an `on_scroll` handler.
    pub fn scroll_target(&self, point: crate::Point) -> Option<RuntimeNodeId> {
        let mut current = self.hit_test(point)?;
        loop {
            let node = self.nodes.get(current)?;
            if node.events.on_scroll.is_some() {
                return Some(current);
            }
            current = node.parent?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{EventState, Mutation, NodeKind};
    use std::rc::Rc;

    fn leaf_at(runtime: &mut Runtime, parent: RuntimeNodeId, rect: crate::Rect) -> RuntimeNodeId {
        let mut tx = runtime.transaction();
        let node = tx.create_node(NodeKind::Container);
        tx.insert_child(parent, node, None);
        drop(tx);
        let n = runtime.nodes.get_mut(node).unwrap();
        n.layout.rect = rect;
        node
    }

    #[test]
    fn transformed_and_clipped_hits_update_without_rebuilding_membership() {
        let mut runtime = Runtime::new();
        let (root, child) = {
            let mut tx = runtime.transaction();
            let root = tx.create_node(NodeKind::Container);
            let child = tx.create_node(NodeKind::Container);
            tx.insert_child(root, child, None);
            tx.apply(Mutation::SetEventHandlers {
                node: child,
                handlers: EventState {
                    on_click: Some(Rc::new(|| {})),
                    ..Default::default()
                },
            });
            tx.apply(Mutation::SetTransform {
                node: root,
                transform: super::super::Transform2D { x: 10.0, y: 0.0 },
            });
            tx.apply(Mutation::SetClipsChildren {
                node: root,
                clips_children: true,
            });
            (root, child)
        };
        runtime.set_root(Some(root));
        runtime.nodes.get_mut(root).unwrap().layout.rect = crate::Rect {
            x: 0.0,
            y: 0.0,
            width: 30.0,
            height: 30.0,
        };
        runtime.nodes.get_mut(child).unwrap().layout.rect = crate::Rect {
            x: 20.0,
            y: 0.0,
            width: 30.0,
            height: 30.0,
        };
        runtime.rebuild_composite();
        runtime.rebuild_hit_test();
        assert_eq!(
            runtime.hit_test(crate::Point { x: 35.0, y: 10.0 }),
            Some(child)
        );
        assert_eq!(runtime.hit_test(crate::Point { x: 45.0, y: 10.0 }), None);

        runtime.transaction().apply(Mutation::SetTransform {
            node: root,
            transform: super::super::Transform2D { x: 20.0, y: 0.0 },
        });
        runtime.rebuild_composite();
        runtime.rebuild_hit_test();
        assert_eq!(runtime.hit_test(crate::Point { x: 35.0, y: 10.0 }), None);
        assert_eq!(
            runtime.hit_test(crate::Point { x: 45.0, y: 10.0 }),
            Some(child)
        );
    }

    fn absolute_leaf_at(
        runtime: &mut Runtime,
        parent: RuntimeNodeId,
        rect: crate::Rect,
    ) -> RuntimeNodeId {
        let node = leaf_at(runtime, parent, rect);
        runtime.nodes.get_mut(node).unwrap().layout_style.position =
            taffy::style::Position::Absolute;
        node
    }

    fn clickable(runtime: &mut Runtime, node: RuntimeNodeId) {
        let mut tx = runtime.transaction();
        tx.apply(Mutation::SetEventHandlers {
            node,
            handlers: EventState {
                on_click: Some(Rc::new(|| {})),
                ..Default::default()
            },
        });
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> crate::Rect {
        crate::Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn a_moved_node_is_found_at_its_new_rect_without_a_rebuild() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        let node = leaf_at(&mut runtime, root, rect(0.0, 0.0, 10.0, 10.0));
        clickable(&mut runtime, node);
        runtime.rebuild_hit_test();

        clickable(&mut runtime, node);
        assert!(!runtime.hit_test_dirty);
        runtime.nodes.get_mut(node).unwrap().layout.rect = rect(50.0, 50.0, 10.0, 10.0);
        runtime.hit_rects.push(node);
        runtime.rebuild_hit_test();
        assert_eq!(
            runtime.hit_test(crate::Point { x: 55.0, y: 55.0 }),
            Some(node)
        );
        assert_eq!(runtime.hit_test(crate::Point { x: 5.0, y: 5.0 }), None);
    }

    #[test]
    fn changing_one_nodes_interactivity_preserves_other_hit_regions() {
        let mut runtime = Runtime::new();
        let root = runtime.transaction().create_node(NodeKind::Container);
        runtime.set_root(Some(root));
        let first = leaf_at(&mut runtime, root, rect(0.0, 0.0, 10.0, 10.0));
        let second = leaf_at(&mut runtime, root, rect(200.0, 0.0, 10.0, 10.0));
        clickable(&mut runtime, first);
        runtime.rebuild_hit_test();
        let first_slot = runtime.nodes.get(first).unwrap().hit_slot;
        let first_cell = runtime.hit_index.cells.get(&(0, 0)).cloned();

        clickable(&mut runtime, second);
        runtime.rebuild_hit_test();
        assert_eq!(runtime.nodes.get(first).unwrap().hit_slot, first_slot);
        assert_eq!(runtime.hit_index.cells.get(&(0, 0)), first_cell.as_ref());
        assert_eq!(
            runtime.hit_test(crate::Point { x: 205.0, y: 5.0 }),
            Some(second)
        );

        runtime.transaction().apply(Mutation::SetEventHandlers {
            node: second,
            handlers: EventState::default(),
        });
        runtime.rebuild_hit_test();
        assert_eq!(runtime.nodes.get(first).unwrap().hit_slot, first_slot);
        assert_eq!(runtime.hit_index.cells.get(&(0, 0)), first_cell.as_ref());
        assert_eq!(runtime.hit_test(crate::Point { x: 205.0, y: 5.0 }), None);
    }

    #[test]
    fn removing_a_middle_hit_slot_keeps_reused_slots_and_paint_order_valid() {
        let mut runtime = Runtime::new();
        let root = runtime.transaction().create_node(NodeKind::Container);
        runtime.set_root(Some(root));
        let first = leaf_at(&mut runtime, root, rect(0.0, 0.0, 10.0, 10.0));
        let second = leaf_at(&mut runtime, root, rect(200.0, 0.0, 10.0, 10.0));
        let third = leaf_at(&mut runtime, root, rect(400.0, 0.0, 10.0, 10.0));
        for node in [first, second, third] {
            clickable(&mut runtime, node);
        }
        runtime.rebuild_hit_test();
        runtime.transaction().apply(Mutation::SetEventHandlers {
            node: second,
            handlers: EventState::default(),
        });
        runtime.rebuild_hit_test();
        assert_eq!(runtime.nodes.get(third).unwrap().hit_slot, Some(1));
        runtime.nodes.get_mut(third).unwrap().layout.rect = rect(0.0, 0.0, 10.0, 10.0);
        runtime.hit_rects.push(third);
        runtime.rebuild_hit_test();
        assert_eq!(
            runtime.hit_test(crate::Point { x: 5.0, y: 5.0 }),
            Some(third)
        );
        assert_eq!(runtime.hit_test(crate::Point { x: 405.0, y: 5.0 }), None);
        clickable(&mut runtime, second);
        runtime.nodes.get_mut(second).unwrap().layout.rect = rect(0.0, 0.0, 10.0, 10.0);
        runtime.rebuild_hit_test();
        assert_eq!(
            runtime.hit_test(crate::Point { x: 5.0, y: 5.0 }),
            Some(third)
        );
        runtime.transaction().remove_subtree(third);
        runtime.rebuild_hit_test();
        assert_eq!(
            runtime.hit_test(crate::Point { x: 5.0, y: 5.0 }),
            Some(second)
        );
    }

    #[test]
    fn root_replacement_and_removal_clear_old_hit_and_focus_entries() {
        let mut runtime = Runtime::new();
        let root = runtime.transaction().create_node(NodeKind::Container);
        runtime.set_root(Some(root));
        let child = leaf_at(&mut runtime, root, rect(0.0, 0.0, 10.0, 10.0));
        runtime.transaction().apply(Mutation::SetEventHandlers {
            node: child,
            handlers: EventState {
                focusable: true,
                ..Default::default()
            },
        });
        runtime.rebuild_hit_test();
        assert_eq!(runtime.next_focus(None, false), Some(child));
        let replacement = runtime.transaction().create_node(NodeKind::Container);
        runtime.nodes.get_mut(replacement).unwrap().layout.rect = rect(0.0, 0.0, 10.0, 10.0);
        clickable(&mut runtime, replacement);
        runtime.set_root(Some(replacement));
        runtime.rebuild_hit_test();
        assert_eq!(runtime.nodes.get(child).unwrap().hit_slot, None);
        assert_eq!(runtime.next_focus(None, false), None);
        assert_eq!(
            runtime.hit_test(crate::Point { x: 5.0, y: 5.0 }),
            Some(replacement)
        );
        runtime.transaction().remove_subtree(replacement);
        runtime.rebuild_hit_test();
        assert_eq!(runtime.root(), None);
        assert!(runtime.hit_entries.is_empty());
        assert!(runtime.hit_index.cells.is_empty());
        assert!(runtime.hit_index.spanning.is_empty());
        assert_eq!(runtime.hit_test(crate::Point { x: 5.0, y: 5.0 }), None);
    }

    #[test]
    fn changing_focusability_preserves_spatial_entries_and_updates_focus_order() {
        let mut runtime = Runtime::new();
        let root = runtime.transaction().create_node(NodeKind::Container);
        runtime.set_root(Some(root));
        let child = leaf_at(&mut runtime, root, rect(0.0, 0.0, 10.0, 10.0));
        clickable(&mut runtime, child);
        runtime.rebuild_hit_test();
        let cells = runtime.hit_index.cells.clone();
        for focusable in [true, false] {
            runtime.transaction().apply(Mutation::SetEventHandlers {
                node: child,
                handlers: EventState {
                    on_click: Some(Rc::new(|| {})),
                    focusable,
                    ..Default::default()
                },
            });
            runtime.rebuild_hit_test();
            assert_eq!(runtime.hit_index.cells, cells);
            assert_eq!(runtime.next_focus(None, false), focusable.then_some(child));
        }
    }

    #[cfg(feature = "perf-metrics")]
    #[test]
    fn membership_updates_touch_one_hit_entry_in_a_wide_tree() {
        let mut runtime = Runtime::new();
        let root = runtime.transaction().create_node(NodeKind::Container);
        runtime.set_root(Some(root));
        let mut children = Vec::new();
        for index in 0..1000 {
            let node = leaf_at(
                &mut runtime,
                root,
                rect(index as f32 * 200.0, 0.0, 10.0, 10.0),
            );
            clickable(&mut runtime, node);
            children.push(node);
        }
        runtime.rebuild_hit_test();
        crate::metrics::reset_frame_metrics();
        runtime.transaction().apply(Mutation::SetEventHandlers {
            node: children[500],
            handlers: EventState::default(),
        });
        runtime.rebuild_hit_test();
        assert_eq!(crate::metrics::frame_metrics().hit_nodes_updated, 1);
        assert_eq!(runtime.hit_entries.len(), 999);
    }

    #[test]
    fn reordering_overlapping_nodes_preserves_their_spatial_entries() {
        let mut runtime = Runtime::new();
        let root = runtime.transaction().create_node(NodeKind::Container);
        runtime.set_root(Some(root));
        let first = leaf_at(&mut runtime, root, rect(0.0, 0.0, 10.0, 10.0));
        let second = leaf_at(&mut runtime, root, rect(0.0, 0.0, 10.0, 10.0));
        clickable(&mut runtime, first);
        clickable(&mut runtime, second);
        runtime.rebuild_hit_test();
        let cells = runtime.hit_index.cells.clone();
        let point = crate::Point { x: 5.0, y: 5.0 };
        assert_eq!(runtime.hit_test(point), Some(second));

        runtime
            .transaction()
            .reorder_children(root, &[second, first]);
        runtime.rebuild_hit_test();
        assert_eq!(runtime.hit_index.cells, cells);
        assert_eq!(runtime.hit_test(point), Some(first));
    }

    #[test]
    fn spatial_hits_match_reverse_paint_order_after_moves() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        let mut nodes = Vec::new();
        for i in 0..240 {
            let wide = i % 31 == 0;
            let node = leaf_at(
                &mut runtime,
                root,
                rect(
                    (i % 24) as f32 * 36.0 - 100.0,
                    (i / 24) as f32 * 44.0 - 70.0,
                    if wide { 8_000.0 } else { (i % 5) as f32 * 12.0 },
                    if wide { 8_000.0 } else { 25.0 },
                ),
            );
            clickable(&mut runtime, node);
            nodes.push(node);
        }
        runtime.rebuild_hit_test();

        let check = |runtime: &Runtime| {
            for y in (-3..20).map(|n| n as f32 * 32.0) {
                for x in (-4..48).map(|n| n as f32 * 32.0) {
                    let point = crate::Point { x, y };
                    let expected = runtime
                        .hit_entries
                        .iter()
                        .enumerate()
                        .filter(|(_, entry)| {
                            entry.rect.width > 0.0
                                && entry.rect.height > 0.0
                                && entry.rect.contains(point)
                        })
                        .max_by_key(|(slot, _)| runtime.hit_ranks[*slot])
                        .map(|(_, entry)| entry.node);
                    assert_eq!(runtime.hit_test(point), expected, "at {point:?}");
                }
            }
        };
        check(&runtime);

        for &node in nodes.iter().step_by(7) {
            runtime.nodes.get_mut(node).unwrap().layout.rect.x += 800.0;
            runtime.hit_rects.push(node);
        }
        runtime.rebuild_hit_test();
        check(&runtime);
    }

    #[test]
    fn moving_between_indexed_and_spanning_regions_updates_hits() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        let node = leaf_at(&mut runtime, root, rect(0.0, 0.0, 10.0, 10.0));
        clickable(&mut runtime, node);
        runtime.rebuild_hit_test();

        runtime.nodes.get_mut(node).unwrap().layout.rect = rect(0.0, 0.0, 10_000.0, 10.0);
        runtime.hit_rects.push(node);
        runtime.rebuild_hit_test();
        assert_eq!(
            runtime.hit_test(crate::Point { x: 9_000.0, y: 5.0 }),
            Some(node)
        );

        runtime.nodes.get_mut(node).unwrap().layout.rect = rect(5_000.0, 0.0, 10.0, 10.0);
        runtime.hit_rects.push(node);
        runtime.rebuild_hit_test();
        assert_eq!(runtime.hit_test(crate::Point { x: 0.0, y: 5.0 }), None);
        assert_eq!(
            runtime.hit_test(crate::Point { x: 5_005.0, y: 5.0 }),
            Some(node)
        );
    }

    #[test]
    fn hit_test_returns_the_topmost_matching_node() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        let back = leaf_at(&mut runtime, root, rect(0.0, 0.0, 100.0, 100.0));
        let front = leaf_at(&mut runtime, root, rect(0.0, 0.0, 100.0, 100.0));

        for node in [back, front] {
            let mut tx = runtime.transaction();
            tx.apply(Mutation::SetEventHandlers {
                node,
                handlers: EventState {
                    on_click: Some(Rc::new(|| {})),
                    ..Default::default()
                },
            });
        }
        runtime.rebuild_hit_test();

        assert_eq!(
            runtime.hit_test(crate::Point { x: 5.0, y: 5.0 }),
            Some(front)
        );
    }

    #[test]
    fn an_absolutely_positioned_node_hit_tests_above_a_later_flow_sibling() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        // Created first (earlier document order), but absolutely
        // positioned — must still win over a plain-flow node created
        // afterward, which a plain depth-first hit list would favor.
        let popover = absolute_leaf_at(&mut runtime, root, rect(0.0, 0.0, 100.0, 100.0));
        let later_flow_sibling = leaf_at(&mut runtime, root, rect(0.0, 0.0, 100.0, 100.0));
        clickable(&mut runtime, popover);
        clickable(&mut runtime, later_flow_sibling);
        runtime.rebuild_hit_test();

        assert_eq!(
            runtime.hit_test(crate::Point { x: 5.0, y: 5.0 }),
            Some(popover)
        );
    }

    #[test]
    fn absolute_z_index_changes_paint_and_hit_order_without_changing_focus_order() {
        let mut runtime = Runtime::new();
        let root = runtime.transaction().create_node(NodeKind::Container);
        runtime.set_root(Some(root));
        let first = absolute_leaf_at(&mut runtime, root, rect(0.0, 0.0, 100.0, 100.0));
        let nested = absolute_leaf_at(&mut runtime, first, rect(0.0, 0.0, 100.0, 100.0));
        let second = absolute_leaf_at(&mut runtime, root, rect(0.0, 0.0, 100.0, 100.0));
        for node in [first, nested, second] {
            runtime.transaction().apply(Mutation::SetEventHandlers {
                node,
                handlers: EventState {
                    on_click: Some(Rc::new(|| {})),
                    focusable: true,
                    ..Default::default()
                },
            });
        }
        let point = crate::Point { x: 5.0, y: 5.0 };
        runtime.rebuild_hit_test();
        assert_eq!(runtime.hit_test(point), Some(second));

        runtime.transaction().apply(Mutation::SetZIndex {
            node: first,
            z_index: 1,
        });
        runtime.rebuild_hit_test();
        assert_eq!(runtime.paint_order(), &[root, second, first, nested]);
        assert_eq!(runtime.hit_test(point), Some(nested));
        assert_eq!(runtime.next_focus(None, false), Some(first));
        assert_eq!(runtime.next_focus(Some(first), false), Some(nested));
        assert_eq!(runtime.next_focus(Some(nested), false), Some(second));

        runtime.transaction().apply(Mutation::SetZIndex {
            node: first,
            z_index: -1,
        });
        runtime.rebuild_hit_test();
        assert_eq!(runtime.hit_test(point), Some(second));
    }

    #[test]
    fn a_node_nested_inside_an_absolute_subtree_also_hit_tests_above_flow() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        let popover = absolute_leaf_at(&mut runtime, root, rect(0.0, 0.0, 100.0, 100.0));
        let popover_child = leaf_at(&mut runtime, popover, rect(0.0, 0.0, 100.0, 100.0));
        let later_flow_sibling = leaf_at(&mut runtime, root, rect(0.0, 0.0, 100.0, 100.0));
        clickable(&mut runtime, popover_child);
        clickable(&mut runtime, later_flow_sibling);
        runtime.rebuild_hit_test();

        assert_eq!(
            runtime.hit_test(crate::Point { x: 5.0, y: 5.0 }),
            Some(popover_child)
        );
    }

    #[test]
    fn focus_order_stays_plain_document_order_regardless_of_position() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        let popover = absolute_leaf_at(&mut runtime, root, rect(0.0, 0.0, 100.0, 100.0));
        let later_flow_sibling = leaf_at(&mut runtime, root, rect(0.0, 0.0, 100.0, 100.0));
        for node in [popover, later_flow_sibling] {
            let mut tx = runtime.transaction();
            tx.apply(Mutation::SetEventHandlers {
                node,
                handlers: EventState {
                    focusable: true,
                    ..Default::default()
                },
            });
        }
        runtime.rebuild_hit_test();

        assert_eq!(runtime.next_focus(None, false), Some(popover));
        assert_eq!(
            runtime.next_focus(Some(popover), false),
            Some(later_flow_sibling)
        );
    }

    #[test]
    fn nodes_without_handlers_are_not_hit_testable() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        leaf_at(&mut runtime, root, rect(0.0, 0.0, 100.0, 100.0));
        runtime.rebuild_hit_test();

        assert_eq!(runtime.hit_test(crate::Point { x: 5.0, y: 5.0 }), None);
    }

    #[test]
    fn set_hovered_to_the_same_node_marks_nothing() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));

        assert!(runtime.set_hovered(Some(root)));
        runtime.nodes.get_mut(root).unwrap().dirty = DirtyFlags::empty();

        assert!(
            !runtime.set_hovered(Some(root)),
            "hovering the already-hovered node must report no change"
        );
        assert!(runtime.nodes.get(root).unwrap().dirty.is_empty());
    }

    #[test]
    fn pointer_capture_overrides_hit_testing() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        let captured = leaf_at(&mut runtime, root, rect(500.0, 500.0, 10.0, 10.0));
        runtime.capture_pointer(captured);

        assert_eq!(
            runtime.route_pointer(crate::Point { x: 0.0, y: 0.0 }),
            Some(captured)
        );
        runtime.release_pointer_capture();
        assert_eq!(runtime.route_pointer(crate::Point { x: 0.0, y: 0.0 }), None);
    }

    #[test]
    fn next_focus_cycles_in_retained_order_and_wraps() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        let mut ids = Vec::new();
        for _ in 0..3 {
            let node = leaf_at(&mut runtime, root, rect(0.0, 0.0, 10.0, 10.0));
            let mut tx = runtime.transaction();
            tx.apply(Mutation::SetEventHandlers {
                node,
                handlers: EventState {
                    focusable: true,
                    ..Default::default()
                },
            });
            ids.push(node);
        }
        runtime.rebuild_hit_test();

        assert_eq!(runtime.next_focus(None, false), Some(ids[0]));
        assert_eq!(runtime.next_focus(Some(ids[0]), false), Some(ids[1]));
        assert_eq!(runtime.next_focus(Some(ids[2]), false), Some(ids[0]));
        assert_eq!(runtime.next_focus(None, true), Some(ids[2]));
    }

    #[test]
    fn scroll_target_walks_up_to_the_nearest_scrollable_ancestor() {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        runtime.set_root(Some(root));
        let scrollable = leaf_at(&mut runtime, root, rect(0.0, 0.0, 100.0, 100.0));
        {
            let mut tx = runtime.transaction();
            tx.apply(Mutation::SetEventHandlers {
                node: scrollable,
                handlers: EventState {
                    on_scroll: Some(Rc::new(|_| {})),
                    ..Default::default()
                },
            });
        }
        let inner = leaf_at(&mut runtime, scrollable, rect(0.0, 0.0, 20.0, 20.0));
        {
            let mut tx = runtime.transaction();
            tx.apply(Mutation::SetEventHandlers {
                node: inner,
                handlers: EventState {
                    on_click: Some(Rc::new(|| {})),
                    ..Default::default()
                },
            });
        }
        runtime.rebuild_hit_test();

        assert_eq!(
            runtime.scroll_target(crate::Point { x: 5.0, y: 5.0 }),
            Some(scrollable)
        );
    }
}
