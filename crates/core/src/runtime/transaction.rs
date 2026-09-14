use super::dirty::DirtyFlags;
use super::mutation::Mutation;
use super::node::{NodeKind, RuntimeNode, RuntimeNodeId, TextNode};
use super::Runtime;

/// Batches mutations against one [`Runtime`] so a compound change (e.g. a
/// drag updating a slider's value, a label, and a thumb position) becomes
/// one set of touched nodes instead of three separate invalidations.
pub struct RuntimeTransaction<'a> {
    runtime: &'a mut Runtime,
    touched: Vec<RuntimeNodeId>,
}

impl<'a> RuntimeTransaction<'a> {
    pub(super) fn new(runtime: &'a mut Runtime) -> Self {
        RuntimeTransaction {
            runtime,
            touched: Vec::new(),
        }
    }

    fn touch(&mut self, id: RuntimeNodeId, flags: DirtyFlags) {
        if let Some(node) = self.runtime.nodes.get_mut(id) {
            node.dirty |= flags;
        }
        if !self.touched.contains(&id) {
            self.touched.push(id);
        }
    }

    pub fn create_node(&mut self, kind: NodeKind) -> RuntimeNodeId {
        let id = self
            .runtime
            .nodes
            .insert_with(|id| RuntimeNode::new(id, kind));
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
            self.touch(previous_parent, DirtyFlags::STRUCTURE);
        }
        if let Some(child_node) = self.runtime.nodes.get_mut(child) {
            child_node.parent = Some(parent);
        }
        if let Some(parent_node) = self.runtime.nodes.get_mut(parent) {
            parent_node.children.insert(child, before);
        }
        self.touch(parent, DirtyFlags::STRUCTURE);
    }

    pub fn remove_subtree(&mut self, root: RuntimeNodeId) {
        let Some((parent, children)) = self
            .runtime
            .nodes
            .get(root)
            .map(|n| (n.parent, n.children.as_slice().to_vec()))
        else {
            return;
        };
        for child in children {
            self.remove_subtree(child);
        }
        self.runtime.nodes.remove(root);
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
                    n.layout_style = style;
                    changed
                });
                if changed {
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
        }
    }

    pub fn touched(&self) -> &[RuntimeNodeId] {
        &self.touched
    }
}
