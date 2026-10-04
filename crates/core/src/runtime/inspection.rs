use super::{DirtyFlags, NodeKind, Runtime, RuntimeNodeId};
use crate::Rect;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct InspectedNode {
    pub id: RuntimeNodeId,
    pub parent: Option<RuntimeNodeId>,
    pub depth: usize,
    pub kind: &'static str,
    pub text: Option<Rc<str>>,
    pub layout_rect: Rect,
    pub visual_rect: Rect,
    pub content_rect: Rect,
    pub paint_bounds: Option<Rect>,
    pub hit_rect: Option<Rect>,
    pub clip: Option<Rect>,
    pub opacity: f32,
    pub focusable: bool,
    pub invalidations: DirtyFlags,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeInspection {
    pub nodes: Vec<InspectedNode>,
    pub layout_epoch: u64,
}

impl Runtime {
    pub fn set_inspection_enabled(&mut self, enabled: bool) {
        if enabled == self.inspection_invalidations.is_some() {
            return;
        }
        self.inspection_invalidations = enabled.then(Default::default);
        if enabled {
            let pending: Vec<_> = self
                .layout_roots
                .iter()
                .copied()
                .map(|id| (id, DirtyFlags::LAYOUT))
                .chain(
                    self.paint_queue
                        .iter()
                        .copied()
                        .map(|id| (id, DirtyFlags::PAINT)),
                )
                .chain(
                    self.composite_queue
                        .iter()
                        .copied()
                        .map(|id| (id, DirtyFlags::COMPOSITE)),
                )
                .chain(
                    self.hit_membership
                        .iter()
                        .copied()
                        .map(|id| (id, DirtyFlags::HIT_TEST)),
                )
                .collect();
            for (id, flags) in pending {
                self.record_invalidation(id, flags);
            }
            if self.paint_order_dirty {
                if let Some(root) = self.root {
                    self.record_invalidation(root, DirtyFlags::STRUCTURE);
                }
            }
        }
    }

    pub(super) fn record_invalidation(&mut self, id: RuntimeNodeId, flags: DirtyFlags) {
        if let Some(invalidations) = self.inspection_invalidations.as_mut() {
            *invalidations.entry(id).or_default() |= flags;
        }
    }

    /// Consumes invalidations accumulated since the previous snapshot.
    pub fn take_inspection(&mut self) -> Option<RuntimeInspection> {
        let invalidations = self.inspection_invalidations.as_mut()?;
        let mut snapshot = RuntimeInspection {
            nodes: Vec::with_capacity(self.nodes.len()),
            layout_epoch: self.layout_epoch,
        };
        let mut stack = Vec::new();
        stack.extend(self.root.map(|root| (root, 0)));
        while let Some((id, depth)) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let translate = |rect: Rect| Rect {
                x: rect.x + node.layout.effective_transform.x,
                y: rect.y + node.layout.effective_transform.y,
                ..rect
            };
            let (kind, text) = match &node.kind {
                NodeKind::Container => ("Container", None),
                NodeKind::Text(text) => ("Text", Some(text.text.clone())),
                NodeKind::Image(_) => ("Image", None),
                NodeKind::Custom(_) => ("Custom", None),
            };
            snapshot.nodes.push(InspectedNode {
                id,
                parent: node.parent,
                depth,
                kind,
                text,
                layout_rect: node.layout.rect,
                visual_rect: translate(node.layout.rect),
                content_rect: translate(node.layout.content_rect),
                paint_bounds: node
                    .paint
                    .fragment
                    .as_ref()
                    .map(|fragment| translate(fragment.bounds)),
                hit_rect: node
                    .hit_slot
                    .map(|slot| self.hit_entries[slot as usize].rect),
                clip: node.layout.effective_clip,
                opacity: node.layout.effective_opacity,
                focusable: node.events.focusable,
                invalidations: invalidations.get(&id).copied().unwrap_or_default(),
            });
            stack.extend(
                node.children
                    .as_slice()
                    .iter()
                    .rev()
                    .map(|&child| (child, depth + 1)),
            );
        }
        invalidations.clear();
        Some(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{EventState, Mutation, Transform2D};
    use crate::{PaintStyle, Size};
    use creamui_theme::{Color, ColorScheme};

    #[test]
    fn snapshots_are_opt_in_and_consume_only_current_invalidations() {
        let mut runtime = Runtime::new();
        let root = runtime.transaction().create_node(NodeKind::Container);
        runtime.set_root(Some(root));
        assert!(runtime.take_inspection().is_none());
        runtime.set_inspection_enabled(true);
        assert!(!runtime.take_inspection().unwrap().nodes[0]
            .invalidations
            .is_empty());
        assert!(runtime.take_inspection().unwrap().nodes[0]
            .invalidations
            .is_empty());
        runtime.transaction().apply(Mutation::SetPaintStyle {
            node: root,
            style: PaintStyle {
                background: Some(Color::rgb(10, 20, 30).into()),
                ..Default::default()
            },
        });
        runtime.rebuild_paint(&ColorScheme::light());
        assert_eq!(
            runtime.take_inspection().unwrap().nodes[0].invalidations,
            DirtyFlags::PAINT
        );
        runtime.set_inspection_enabled(false);
        assert!(runtime.take_inspection().is_none());
    }

    #[test]
    fn snapshot_preserves_document_order_and_composited_geometry() {
        let mut runtime = Runtime::new();
        runtime.set_inspection_enabled(true);
        let (root, child) = {
            let mut tx = runtime.transaction();
            let root = tx.create_node(NodeKind::Container);
            let child = tx.create_node(NodeKind::Text(super::super::TextNode {
                text: "hello".into(),
            }));
            tx.insert_child(root, child, None);
            for node in [root, child] {
                tx.apply(Mutation::SetLayoutStyle {
                    node,
                    style: crate::layout::Style {
                        size: crate::layout::Size {
                            width: crate::layout::Dimension::Length(50.0),
                            height: crate::layout::Dimension::Length(50.0),
                        },
                        ..Default::default()
                    },
                });
            }
            tx.apply(Mutation::SetTransform {
                node: root,
                transform: Transform2D { x: 10.0, y: 20.0 },
            });
            tx.apply(Mutation::SetEventHandlers {
                node: child,
                handlers: EventState {
                    focusable: true,
                    ..Default::default()
                },
            });
            (root, child)
        };
        runtime.set_root(Some(root));
        runtime.compute_layout(Size {
            width: 100.0,
            height: 100.0,
        });
        runtime.rebuild_paint(&ColorScheme::light());
        runtime.rebuild_composite();
        runtime.rebuild_hit_test();
        let snapshot = runtime.take_inspection().unwrap();
        assert_eq!(
            snapshot
                .nodes
                .iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            [root, child]
        );
        let child = &snapshot.nodes[1];
        assert_eq!(child.depth, 1);
        assert_eq!(child.parent, Some(root));
        assert_eq!(child.text.as_deref(), Some("hello"));
        assert_eq!(child.visual_rect.x, child.layout_rect.x + 10.0);
        assert_eq!(child.visual_rect.y, child.layout_rect.y + 20.0);
        assert_eq!(child.hit_rect, Some(child.visual_rect));
        assert!(child
            .invalidations
            .contains(DirtyFlags::LAYOUT | DirtyFlags::HIT_TEST));
    }
}
