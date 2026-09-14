//! The persistent runtime tree: the structure meant to eventually replace
//! `Instance`/`BoxedWidget` reconciliation as the engine's source of
//! truth. Built alongside the existing `scene` module, not wired into
//! `Renderer` yet — see [`mount_legacy_widget`] for the compatibility
//! bridge between the two.

mod arena;
mod dirty;
mod mount;
mod mutation;
mod node;
mod transaction;

pub use dirty::DirtyFlags;
pub use mount::mount_legacy_widget;
pub use mutation::{Mutation, Transform2D};
pub use node::{Children, CustomNode, ImageNode, NodeKind, RuntimeNode, RuntimeNodeId, TextNode};
pub use transaction::RuntimeTransaction;

use arena::Arena;

pub struct Runtime {
    nodes: Arena<RuntimeNode>,
    root: Option<RuntimeNodeId>,
}

impl Runtime {
    pub fn new() -> Self {
        Runtime {
            nodes: Arena::new(),
            root: None,
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

    /// Panics on the first broken invariant found: a child whose `parent`
    /// doesn't point back, a child claimed by more than one parent, or a
    /// root set to an id this runtime doesn't hold. Intended for tests and
    /// debug assertions, not the hot path.
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
