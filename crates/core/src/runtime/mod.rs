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
pub use transaction::RuntimeTransaction;
pub use view::{IntoView, View};

use arena::Arena;
use taffy::TaffyTree;

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
    /// Set when a mutation marks any node's HIT_TEST/STRUCTURE dirty; lets
    /// [`Runtime::rebuild_hit_test`] skip the tree walk otherwise.
    hit_test_dirty: bool,
    hit_entries: Vec<HitEntry>,
    focus_order: Vec<RuntimeNodeId>,
    pointer: PointerState,
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
            hit_test_dirty: false,
            hit_entries: Vec::new(),
            focus_order: Vec::new(),
            pointer: PointerState::default(),
        }
    }

    pub fn transaction(&mut self) -> RuntimeTransaction<'_> {
        RuntimeTransaction::new(self)
    }

    pub fn get(&self, id: RuntimeNodeId) -> Option<&RuntimeNode> {
        self.nodes.get(id)
    }

    pub fn root(&self) -> Option<RuntimeNodeId> {
        self.root
    }

    pub fn set_root(&mut self, id: Option<RuntimeNodeId>) {
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
        self.sync_layout_rects(root)
    }

    /// Iterative top-down pass (a deep tree can overflow the stack under
    /// recursion) computing each node's window-space rect from `taffy`'s
    /// parent-relative output.
    fn sync_layout_rects(&mut self, root: RuntimeNodeId) -> Vec<crate::Rect> {
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
            if node.layout.rect != rect {
                damage.push(node.layout.rect);
                damage.push(rect);
                node.layout.previous_rect = node.layout.rect;
                node.layout.rect = rect;
                node.layout.last_layout_epoch = self.layout_epoch;
                node.dirty |= DirtyFlags::PAINT | DirtyFlags::HIT_TEST;
                self.hit_test_dirty = true;
            }

            stack.extend(children.into_iter().map(|child| (child, child_origin)));
        }
        damage
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
