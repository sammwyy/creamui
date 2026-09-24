use super::dirty::DirtyFlags;
use super::node::RuntimeNodeId;
use super::Runtime;

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
    /// Rebuilds the retained hit-test list and focus order from scratch if
    /// (and only if) something marked either dirty since the last call.
    ///
    /// `focus_order` is always plain depth-first document order, unaffected
    /// by position — tab order doesn't follow paint order. `hit_entries`
    /// instead collects every normal-flow node first, then every
    /// absolutely positioned subtree's nodes (in encounter order),
    /// mirroring the legacy `Scene`'s deferred Flow/Absolute two-pass
    /// paint — [`Runtime::hit_test`] scans `hit_entries` in reverse, so an
    /// absolutely positioned node's entries, being last, always win over a
    /// flow sibling's regardless of tree depth/order. A node nested inside
    /// an already-deferred absolute subtree is not independently deferred
    /// again — its whole ancestor subtree already moved as one unit.
    ///
    /// When only listed nodes moved, their entries' rects are patched in
    /// place instead.
    pub fn rebuild_hit_test(&mut self) {
        if !self.hit_test_dirty {
            for id in self.hit_rects.drain(..) {
                if let Some(node) = self.nodes.get(id) {
                    if let Some(slot) = node.hit_slot {
                        self.hit_entries[slot as usize].rect = node.layout.rect;
                    }
                }
            }
            return;
        }
        self.hit_rects.clear();
        for entry in self.hit_entries.drain(..) {
            if let Some(node) = self.nodes.get_mut(entry.node) {
                node.hit_slot = None;
            }
        }
        self.focus_order.clear();
        let Some(root) = self.root else {
            self.hit_test_dirty = false;
            return;
        };

        let mut deferred_absolute: Vec<RuntimeNodeId> = Vec::new();
        let mut stack: Vec<(RuntimeNodeId, bool)> = vec![(root, false)];
        while let Some((id, inside_absolute)) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let is_absolute_root =
                !inside_absolute && node.layout_style.position == taffy::style::Position::Absolute;
            if is_absolute_root {
                deferred_absolute.push(id);
            }
            if !inside_absolute && !is_absolute_root && node.events.is_interactive() {
                self.hit_entries.push(HitEntry {
                    node: id,
                    rect: node.layout.rect,
                });
            }
            if node.events.focusable {
                self.focus_order.push(id);
            }
            let inside_absolute = inside_absolute || is_absolute_root;
            stack.extend(
                node.children
                    .as_slice()
                    .iter()
                    .rev()
                    .map(|&child| (child, inside_absolute)),
            );
        }

        for absolute_root in deferred_absolute {
            let mut stack = vec![absolute_root];
            while let Some(id) = stack.pop() {
                let Some(node) = self.nodes.get(id) else {
                    continue;
                };
                if node.events.is_interactive() {
                    self.hit_entries.push(HitEntry {
                        node: id,
                        rect: node.layout.rect,
                    });
                }
                stack.extend(node.children.as_slice().iter().rev());
            }
        }

        for (slot, entry) in self.hit_entries.iter().enumerate() {
            if let Some(node) = self.nodes.get_mut(entry.node) {
                node.hit_slot = Some(slot as u32);
            }
        }
        self.hit_test_dirty = false;
    }

    /// Topmost interactive node containing `point`, from the retained hit
    /// list built by the last [`Runtime::rebuild_hit_test`].
    pub fn hit_test(&self, point: crate::Point) -> Option<RuntimeNodeId> {
        self.hit_entries
            .iter()
            .rev()
            .find(|entry| entry.rect.contains(point))
            .map(|entry| entry.node)
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
        if let Some(n) = self.nodes.get_mut(node) {
            n.dirty |= DirtyFlags::PAINT;
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
