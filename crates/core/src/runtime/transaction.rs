use super::dirty::DirtyFlags;
use super::mutation::Mutation;
use super::node::{NodeKind, RuntimeNode, RuntimeNodeId, TextNode};
use super::Runtime;
use std::cell::RefCell;
use std::rc::Rc;
use taffy::geometry::Size;
use taffy::style::AvailableSpace;

/// Wraps `measure` so a repeat call with the exact `(known_dimensions,
/// available_space)` pair it was last called with returns the cached
/// result instead of recomputing — `taffy`'s flex/grid algorithms often
/// query the same leaf more than once per `compute_layout` pass (e.g. to
/// resolve flex-basis before the final pass) with identical inputs.
/// Remembers only the single most recent call, since a leaf is not
/// meaningfully queried with more than a couple of distinct inputs within
/// one pass.
fn memoize_measure(measure: crate::MeasureFn) -> crate::MeasureFn {
    let cache: Rc<RefCell<Option<(Size<Option<f32>>, Size<AvailableSpace>, Size<f32>)>>> =
        Rc::new(RefCell::new(None));
    Box::new(move |known_dimensions, available_space| {
        if let Some((cached_known, cached_space, cached_size)) = *cache.borrow() {
            if cached_known == known_dimensions && cached_space == available_space {
                return cached_size;
            }
        }
        let size = measure(known_dimensions, available_space);
        *cache.borrow_mut() = Some((known_dimensions, available_space, size));
        size
    })
}

/// Batches mutations against one [`Runtime`] so a compound change (e.g. a
/// drag updating a slider's value, a label, and a thumb position) becomes
/// one set of touched nodes instead of three separate invalidations.
pub struct RuntimeTransaction<'a> {
    runtime: &'a mut Runtime,
    touched: Vec<RuntimeNodeId>,
    stamp: u64,
}

impl<'a> RuntimeTransaction<'a> {
    pub(super) fn new(runtime: &'a mut Runtime, stamp: u64) -> Self {
        RuntimeTransaction {
            runtime,
            touched: Vec::new(),
            stamp,
        }
    }

    fn touch(&mut self, id: RuntimeNodeId, flags: DirtyFlags) {
        match self.runtime.nodes.get_mut(id) {
            Some(node) => {
                let newly_paint_dirty =
                    flags.contains(DirtyFlags::PAINT) && !node.dirty.contains(DirtyFlags::PAINT);
                let newly_composite_dirty = flags.contains(DirtyFlags::COMPOSITE)
                    && !node.dirty.contains(DirtyFlags::COMPOSITE);
                node.dirty |= flags;
                if newly_paint_dirty {
                    self.runtime.paint_queue.push(id);
                }
                if newly_composite_dirty {
                    self.runtime.composite_queue.push(id);
                }
                if node.touched_stamp != self.stamp {
                    node.touched_stamp = self.stamp;
                    self.touched.push(id);
                }
            }
            None => {
                if !self.touched.contains(&id) {
                    self.touched.push(id);
                }
            }
        }
        if flags.intersects(DirtyFlags::LAYOUT | DirtyFlags::STRUCTURE) {
            self.runtime.layout_dirty = true;
        }
        if flags.intersects(DirtyFlags::HIT_TEST | DirtyFlags::STRUCTURE) {
            self.runtime.hit_test_dirty = true;
        }
        if flags.contains(DirtyFlags::STRUCTURE) {
            self.runtime.paint_order_dirty = true;
        }
    }

    fn sync_taffy_children(&mut self, parent: RuntimeNodeId) {
        let Some(parent_node) = self.runtime.nodes.get(parent) else {
            return;
        };
        let parent_taffy = parent_node.layout.taffy_node;
        let taffy_children: Vec<taffy::NodeId> = parent_node
            .children
            .as_slice()
            .iter()
            .filter_map(|&id| self.runtime.nodes.get(id).map(|n| n.layout.taffy_node))
            .collect();
        let _ = self
            .runtime
            .taffy
            .set_children(parent_taffy, &taffy_children);
        #[cfg(feature = "perf-metrics")]
        crate::metrics::record(|m| m.taffy_children_writes += 1);
    }

    pub fn create_node(&mut self, kind: NodeKind) -> RuntimeNodeId {
        let taffy_node = self
            .runtime
            .taffy
            .new_leaf(taffy::style::Style::default())
            .expect("taffy leaf creation is infallible for a default style");
        #[cfg(feature = "perf-metrics")]
        crate::metrics::record(|m| m.taffy_style_writes += 1);
        let id = self
            .runtime
            .nodes
            .insert_with(|id| RuntimeNode::new(id, kind, taffy_node));
        self.runtime
            .nodes
            .get_mut(id)
            .expect("just inserted")
            .touched_stamp = self.stamp;
        self.runtime.layout_dirty = true;
        self.runtime.paint_order_dirty = true;
        self.touched.push(id);
        id
    }

    /// Moves `child` under `parent`, detaching it from its current parent
    /// first if it has one. Does not defend against cycles — the caller
    /// must not make a node a descendant of itself.
    pub fn insert_child(
        &mut self,
        parent: RuntimeNodeId,
        child: RuntimeNodeId,
        before: Option<RuntimeNodeId>,
    ) {
        let previous_parent = self.runtime.nodes.get(child).and_then(|n| n.parent);
        if let Some(previous_parent) = previous_parent.filter(|&p| p != parent) {
            if let Some(previous_parent_node) = self.runtime.nodes.get_mut(previous_parent) {
                previous_parent_node.children.remove(child);
            }
            self.sync_taffy_children(previous_parent);
            self.touch(previous_parent, DirtyFlags::STRUCTURE);
        }
        if let Some(child_node) = self.runtime.nodes.get_mut(child) {
            child_node.parent = Some(parent);
        }
        if let Some(parent_node) = self.runtime.nodes.get_mut(parent) {
            parent_node.children.insert(child, before);
        }
        self.sync_taffy_children(parent);
        self.touch(parent, DirtyFlags::STRUCTURE);
    }

    /// Replaces `parent`'s entire child list with `ordered` in one pass —
    /// an O(n) alternative to calling [`RuntimeTransaction::insert_child`]
    /// once per item when every item's final position is already known, as
    /// after diffing a keyed list. Every id in `ordered` must already
    /// belong to this runtime; each has its `parent` set to `parent`
    /// unconditionally, with no detach-from-previous-parent step.
    pub fn reorder_children(&mut self, parent: RuntimeNodeId, ordered: &[RuntimeNodeId]) {
        for &child in ordered {
            if let Some(child_node) = self.runtime.nodes.get_mut(child) {
                child_node.parent = Some(parent);
            }
        }
        if let Some(parent_node) = self.runtime.nodes.get_mut(parent) {
            parent_node.children = super::node::Children::from(ordered.to_vec());
        }
        self.sync_taffy_children(parent);
        self.touch(parent, DirtyFlags::STRUCTURE);
    }

    pub fn remove_subtree(&mut self, root: RuntimeNodeId) {
        let Some((parent, children, taffy_node)) = self.runtime.nodes.get(root).map(|n| {
            (
                n.parent,
                n.children.as_slice().to_vec(),
                n.layout.taffy_node,
            )
        }) else {
            return;
        };
        for child in children {
            self.remove_subtree(child);
        }
        self.runtime.nodes.remove(root);
        let _ = self.runtime.taffy.remove(taffy_node);
        if let Some(parent) = parent {
            if let Some(parent_node) = self.runtime.nodes.get_mut(parent) {
                parent_node.children.remove(root);
            }
            self.touch(parent, DirtyFlags::STRUCTURE);
        }
    }

    pub fn apply(&mut self, mutation: Mutation) {
        match mutation {
            Mutation::SetLayoutStyle { node, style } => {
                let changed = self.runtime.nodes.get_mut(node).is_some_and(|n| {
                    let changed = n.layout_style != style;
                    n.layout_style = style.clone();
                    changed
                });
                if changed {
                    if let Some(taffy_node) =
                        self.runtime.nodes.get(node).map(|n| n.layout.taffy_node)
                    {
                        let _ = self.runtime.taffy.set_style(taffy_node, style);
                        #[cfg(feature = "perf-metrics")]
                        crate::metrics::record(|m| m.taffy_style_writes += 1);
                    }
                    self.touch(node, DirtyFlags::LAYOUT);
                }
            }
            Mutation::SetPaintStyle { node, style } => {
                let changed = self.runtime.nodes.get_mut(node).is_some_and(|n| {
                    let changed = n.paint_style != style;
                    n.paint_style = style;
                    changed
                });
                if changed {
                    self.touch(node, DirtyFlags::PAINT);
                }
            }
            Mutation::SetTypographyStyle { node, style } => {
                let changed = self.runtime.nodes.get_mut(node).is_some_and(|n| {
                    let changed = n.typography_style != style;
                    n.typography_style = style;
                    changed
                });
                if changed {
                    self.touch(node, DirtyFlags::PAINT | DirtyFlags::MEASURE);
                }
            }
            Mutation::SetText { node, text } => {
                let changed = self
                    .runtime
                    .nodes
                    .get_mut(node)
                    .is_some_and(|n| match &mut n.kind {
                        NodeKind::Text(existing) if existing.text == text => false,
                        NodeKind::Text(existing) => {
                            existing.text = text;
                            true
                        }
                        other => {
                            *other = NodeKind::Text(TextNode { text });
                            true
                        }
                    });
                if changed {
                    self.touch(node, DirtyFlags::PAINT | DirtyFlags::MEASURE);
                }
            }
            Mutation::SetTransform { node, transform } => {
                let changed = self.runtime.nodes.get_mut(node).is_some_and(|n| {
                    let changed = n.transform != transform;
                    n.transform = transform;
                    changed
                });
                if changed {
                    self.touch(node, DirtyFlags::COMPOSITE);
                }
            }
            Mutation::SetMeasure {
                node,
                measure,
                fingerprint,
            } => {
                let Some(taffy_node) = self.runtime.nodes.get(node).map(|n| n.layout.taffy_node)
                else {
                    return;
                };
                let skip = fingerprint.is_some()
                    && self
                        .runtime
                        .nodes
                        .get(node)
                        .is_some_and(|n| n.layout.measure_fingerprint == fingerprint);
                if skip {
                    return;
                }
                let measure = measure.map(memoize_measure);
                let _ = self.runtime.taffy.set_node_context(taffy_node, measure);
                #[cfg(feature = "perf-metrics")]
                crate::metrics::record(|m| m.taffy_context_writes += 1);
                if let Some(n) = self.runtime.nodes.get_mut(node) {
                    n.layout.measure_fingerprint = fingerprint;
                }
                self.touch(node, DirtyFlags::MEASURE | DirtyFlags::LAYOUT);
            }
            Mutation::SetEventHandlers { node, handlers } => {
                if let Some(n) = self.runtime.nodes.get_mut(node) {
                    n.events = handlers;
                    self.touch(node, DirtyFlags::HIT_TEST);
                }
            }
        }
    }

    pub fn touched(&self) -> &[RuntimeNodeId] {
        &self.touched
    }
}

#[cfg(test)]
mod memoize_measure_tests {
    use super::*;
    use std::cell::Cell;

    fn dims(width: Option<f32>, height: Option<f32>) -> Size<Option<f32>> {
        Size { width, height }
    }

    fn space(width: f32, height: f32) -> Size<AvailableSpace> {
        Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::Definite(height),
        }
    }

    #[test]
    fn an_identical_repeat_query_hits_the_cache() {
        let calls = Rc::new(Cell::new(0));
        let counted = calls.clone();
        let measure = memoize_measure(Box::new(move |_, _| {
            counted.set(counted.get() + 1);
            Size {
                width: 10.0,
                height: 20.0,
            }
        }));

        let first = measure(dims(None, None), space(100.0, 100.0));
        let second = measure(dims(None, None), space(100.0, 100.0));

        assert_eq!(first, second);
        assert_eq!(
            calls.get(),
            1,
            "the second identical query must not recompute"
        );
    }

    #[test]
    fn a_query_with_different_inputs_recomputes() {
        let calls = Rc::new(Cell::new(0));
        let counted = calls.clone();
        let measure = memoize_measure(Box::new(move |_, available_space| {
            counted.set(counted.get() + 1);
            Size {
                width: match available_space.width {
                    AvailableSpace::Definite(w) => w,
                    _ => 0.0,
                },
                height: 20.0,
            }
        }));

        let first = measure(dims(None, None), space(100.0, 100.0));
        let second = measure(dims(None, None), space(200.0, 100.0));

        assert_ne!(first, second);
        assert_eq!(calls.get(), 2);
    }
}
