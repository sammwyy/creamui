use creamui_reactive::{Owner, Signal};
use std::rc::Rc;

use super::binding::{create_binding, SharedRuntime};
use super::branch::create_branch;
use super::keyed::create_keyed_list;
use super::mutation::Mutation;
use super::node::{EventState, ImageNode, NodeKind, RuntimeNodeId, TextNode};
use super::transaction::RuntimeTransaction;

/// Compiles a JSX-like mount call sequence against a [`super::Runtime`]:
/// `cx.container()`/`cx.text(...)` mount static structure once, `cx.bind`/
/// `cx.branch`/`cx.keyed` set up bindings that mutate the already-mounted
/// tree directly on every later reactive change. Nothing here ever
/// rebuilds a widget tree — see REFACTOR.md's Phase 4 objective.
///
/// Cheap to clone: owns only a [`SharedRuntime`] handle, an [`Owner`], and
/// the [`RuntimeNodeId`] new children are appended under.
#[derive(Clone)]
pub struct MountCx {
    runtime: SharedRuntime,
    owner: Owner,
    parent: RuntimeNodeId,
}

impl MountCx {
    pub fn new(runtime: SharedRuntime, owner: Owner, parent: RuntimeNodeId) -> Self {
        MountCx {
            runtime,
            owner,
            parent,
        }
    }

    pub fn runtime(&self) -> &SharedRuntime {
        &self.runtime
    }

    pub fn owner(&self) -> &Owner {
        &self.owner
    }

    pub fn parent(&self) -> RuntimeNodeId {
        self.parent
    }

    /// A cx anchored at `node` instead of `self.parent`, sharing this cx's
    /// runtime and owner — for appending a just-mounted node's own
    /// children.
    pub fn with_parent(&self, node: RuntimeNodeId) -> MountCx {
        MountCx {
            runtime: self.runtime.clone(),
            owner: self.owner.clone(),
            parent: node,
        }
    }

    /// A cx sharing this cx's runtime and parent, but scoped under a fresh
    /// child [`Owner`] — for a component mounting its own bindings/branches
    /// that should dispose together, independent of siblings.
    pub fn with_child_owner(&self) -> MountCx {
        MountCx {
            runtime: self.runtime.clone(),
            owner: self.owner.child(),
            parent: self.parent,
        }
    }

    fn create_and_append(&self, kind: NodeKind) -> RuntimeNodeId {
        self.runtime.transaction(|tx| {
            let id = tx.create_node(kind);
            tx.insert_child(self.parent, id, None);
            id
        })
    }

    pub fn container(&self) -> RuntimeNodeId {
        self.create_and_append(NodeKind::Container)
    }

    pub fn text(&self, text: impl Into<Rc<str>>) -> RuntimeNodeId {
        self.create_and_append(NodeKind::Text(TextNode { text: text.into() }))
    }

    pub fn image(&self, source: impl Into<Rc<str>>) -> RuntimeNodeId {
        self.create_and_append(NodeKind::Image(ImageNode {
            source: source.into(),
        }))
    }

    pub fn set_events(&self, node: RuntimeNodeId, handlers: EventState) {
        self.runtime.transaction(|tx| {
            tx.apply(Mutation::SetEventHandlers { node, handlers });
        });
    }

    /// Ties a reactive read to a mutation of the tree, owned by this cx's
    /// [`Owner`] — see [`create_binding`].
    pub fn bind(&self, f: impl FnMut(&mut RuntimeTransaction) + 'static) {
        create_binding(&self.owner, self.runtime.clone(), f);
    }

    /// Mounts/unmounts a subtree under this cx's parent as `condition`
    /// changes — see [`create_branch`]. `mount` receives a cx anchored at
    /// the same parent and a fresh owner scope for the branch's content.
    pub fn branch(
        &self,
        condition: Signal<bool>,
        mount: impl Fn(&MountCx) -> RuntimeNodeId + 'static,
    ) {
        let anchor = self.clone();
        create_branch(
            &self.owner,
            self.runtime.clone(),
            self.parent,
            condition,
            move |runtime, branch_owner| {
                let branch_cx = MountCx {
                    runtime: runtime.clone(),
                    owner: branch_owner.clone(),
                    parent: anchor.parent,
                };
                mount(&branch_cx)
            },
        );
    }

    /// A keyed, order-preserving list under this cx's parent — see
    /// [`create_keyed_list`]. `render` receives a cx anchored at the same
    /// parent and a fresh owner scope for that item.
    pub fn keyed<T, K>(
        &self,
        items: Signal<Vec<T>>,
        key: impl Fn(&T) -> K + 'static,
        render: impl Fn(&MountCx, &T) -> RuntimeNodeId + 'static,
    ) where
        T: Clone + 'static,
        K: std::hash::Hash + Eq + Clone + 'static,
    {
        let anchor = self.clone();
        create_keyed_list(
            &self.owner,
            self.runtime.clone(),
            self.parent,
            items,
            key,
            move |runtime, item_owner, item| {
                let item_cx = MountCx {
                    runtime: runtime.clone(),
                    owner: item_owner.clone(),
                    parent: anchor.parent,
                };
                render(&item_cx, item)
            },
        );
    }

    /// Registers `f` to run once this cx's owner is disposed.
    pub fn on_cleanup(&self, f: impl FnOnce() + 'static) {
        self.owner.on_cleanup(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Mutation, Runtime};
    use crate::PaintStyle;
    use creamui_theme::Color;

    fn root_cx() -> (SharedRuntime, RuntimeNodeId, MountCx) {
        let mut runtime = Runtime::new();
        let root = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        let runtime = SharedRuntime::new(runtime);
        let cx = MountCx::new(runtime.clone(), Owner::new(), root);
        (runtime, root, cx)
    }

    #[test]
    fn container_text_and_image_append_under_the_parent() {
        let (runtime, root, cx) = root_cx();
        let container = cx.container();
        let text = cx.text("hello");
        let image = cx.image("icon.png");

        let children = runtime.with(|r| r.get(root).unwrap().children.as_slice().to_vec());
        assert_eq!(children, vec![container, text, image]);
        assert!(matches!(
            runtime.with(|r| r.get(text).unwrap().kind.clone()),
            NodeKind::Text(t) if &*t.text == "hello"
        ));
    }

    #[test]
    fn set_events_registers_handlers_the_hit_test_picks_up() {
        let (runtime, root, cx) = root_cx();
        let node = cx.container();
        runtime.transaction(|tx| {
            tx.apply(crate::runtime::Mutation::SetLayoutStyle {
                node,
                style: fixed_size_style(10.0, 10.0),
            });
        });
        cx.set_events(
            node,
            EventState {
                on_click: Some(std::rc::Rc::new(|| {})),
                ..Default::default()
            },
        );
        runtime.with_mut(|r| {
            r.set_root(Some(root));
            r.compute_layout(crate::Size {
                width: 100.0,
                height: 100.0,
            });
            r.rebuild_hit_test();
        });

        assert_eq!(
            runtime.with(|r| r.hit_test(crate::Point { x: 1.0, y: 1.0 })),
            Some(node)
        );
    }

    fn fixed_size_style(width: f32, height: f32) -> taffy::style::Style {
        taffy::style::Style {
            size: taffy::geometry::Size {
                width: taffy::style::Dimension::Length(width),
                height: taffy::style::Dimension::Length(height),
            },
            ..Default::default()
        }
    }

    #[test]
    fn bind_updates_the_tree_without_rebuilding_it() {
        let (runtime, _root, cx) = root_cx();
        let node = cx.container();

        let color = Signal::new(1u8);
        cx.bind({
            let color = color.clone();
            move |tx| {
                tx.apply(Mutation::SetPaintStyle {
                    node,
                    style: PaintStyle {
                        background: Some(Color::rgb(color.get(), 0, 0).into()),
                        ..Default::default()
                    },
                });
            }
        });

        color.set(9);
        assert_eq!(
            runtime.with(|r| r.get(node).unwrap().paint_style.background),
            Some(Color::rgb(9, 0, 0).into())
        );
    }

    #[test]
    fn branch_mounts_nested_containers_through_the_same_cx_style() {
        let (runtime, root, cx) = root_cx();
        let open = Signal::new(false);
        cx.branch(open.clone(), |cx| {
            let panel = cx.container();
            let panel_cx = cx.with_parent(panel);
            panel_cx.text("panel content");
            panel
        });

        assert!(runtime.with(|r| r.get(root).unwrap().children.is_empty()));
        open.set(true);

        let panel = runtime.with(|r| r.get(root).unwrap().children.as_slice()[0]);
        assert_eq!(runtime.with(|r| r.get(panel).unwrap().children.len()), 1);
    }

    #[test]
    fn keyed_mounts_one_node_per_item_through_the_same_cx_style() {
        let (runtime, root, cx) = root_cx();
        let items = Signal::new(vec![1u32, 2, 3]);
        cx.keyed(items, |&id| id, |cx, &id| cx.text(id.to_string()));

        assert_eq!(runtime.with(|r| r.get(root).unwrap().children.len()), 3);
    }
}
