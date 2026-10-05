use super::{Mutation, NodeKind, RuntimeNodeId, SharedRuntime};
use std::collections::HashSet;

pub(super) struct ChildRegion {
    runtime: SharedRuntime,
    parent: RuntimeNodeId,
    anchor: RuntimeNodeId,
    roots: Vec<RuntimeNodeId>,
}

impl ChildRegion {
    pub fn new(runtime: SharedRuntime, parent: RuntimeNodeId) -> Self {
        let anchor = runtime.transaction(|tx| {
            let anchor = tx.create_node(NodeKind::Container);
            tx.apply(Mutation::SetLayoutStyle {
                node: anchor,
                style: taffy::style::Style {
                    display: taffy::style::Display::None,
                    ..Default::default()
                },
            });
            tx.insert_child(parent, anchor, None);
            anchor
        });
        Self {
            runtime,
            parent,
            anchor,
            roots: Vec::new(),
        }
    }

    pub fn anchor(&self) -> RuntimeNodeId {
        self.anchor
    }

    pub fn is_live(&self) -> bool {
        self.runtime
            .with(|runtime| runtime.get(self.anchor).is_some())
    }

    pub fn replace(&mut self, roots: Vec<RuntimeNodeId>) {
        let owned: HashSet<_> = self.roots.iter().chain(&roots).copied().collect();
        let ordered = self.runtime.with(|runtime| {
            let parent = runtime.get(self.parent)?;
            let mut ordered = Vec::with_capacity(parent.children.len() + roots.len());
            for &child in parent.children.as_slice() {
                if child == self.anchor {
                    ordered.extend_from_slice(&roots);
                }
                if !owned.contains(&child) {
                    ordered.push(child);
                }
            }
            (ordered.as_slice() != parent.children.as_slice()).then_some(ordered)
        });
        if let Some(ordered) = ordered {
            self.runtime
                .transaction(|tx| tx.reorder_children(self.parent, &ordered));
        }
        self.roots = roots;
    }

    pub fn remove_roots(&mut self) {
        self.runtime
            .transaction(|tx| tx.remove_subtrees(&self.roots));
        self.roots.clear();
    }

    pub fn clear(&mut self) {
        self.roots.push(self.anchor);
        self.runtime
            .transaction(|tx| tx.remove_subtrees(&self.roots));
        self.roots.clear();
    }
}
