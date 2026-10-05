use super::mount::mount_legacy_widget;
use super::mount_cx::MountCx;
use super::node::RuntimeNodeId;
use crate::BoxedWidget;

/// A mounted node, a deferred mount, or a compatible widget subtree.
pub enum View {
    Mounted(RuntimeNodeId),
    Deferred(Box<dyn FnOnce(&MountCx) -> RuntimeNodeId>),
    Legacy(BoxedWidget),
}

impl View {
    pub fn new(mount: impl FnOnce(&MountCx) -> RuntimeNodeId + 'static) -> Self {
        Self::Deferred(Box::new(mount))
    }

    pub fn mount(self, cx: &MountCx) -> RuntimeNodeId {
        let node = match self {
            View::Mounted(id) => id,
            View::Deferred(mount) => mount(cx),
            View::Legacy(widget) => cx
                .runtime()
                .transaction(|tx| mount_legacy_widget(tx, widget, None)),
        };
        cx.attach(node);
        node
    }
}

/// Owns a retained view's runtime root and disposable binding scope.
pub struct MountedView {
    runtime: super::SharedRuntime,
    owner: creamui_reactive::Owner,
    root: RuntimeNodeId,
}

impl MountedView {
    pub fn new(view: impl IntoView) -> Self {
        Self::mount_into(super::SharedRuntime::new(super::Runtime::new()), view)
    }

    /// Mounts into an empty runtime. The root remains stable across view updates.
    pub fn mount_into(runtime: super::SharedRuntime, view: impl IntoView) -> Self {
        assert!(
            runtime.with(super::Runtime::is_empty),
            "owned view requires an empty runtime"
        );
        let root = runtime.transaction(|tx| {
            let root = tx.create_node(super::NodeKind::Container);
            tx.apply(super::Mutation::SetLayoutStyle {
                node: root,
                style: crate::Style::new().width("100%").height("100%").layout,
            });
            root
        });
        runtime.with_mut(|runtime| runtime.set_root(Some(root)));
        let owner = creamui_reactive::Owner::new();
        owner.on_cleanup({
            let runtime = runtime.clone();
            move || runtime.transaction(|tx| tx.remove_subtree(root))
        });
        let mounted = Self {
            runtime,
            owner,
            root,
        };
        creamui_reactive::untrack(|| {
            view.into_view().mount(&MountCx::new(
                mounted.runtime.clone(),
                mounted.owner.clone(),
                root,
            ));
        });
        mounted
    }

    pub fn runtime(&self) -> &super::SharedRuntime {
        &self.runtime
    }

    pub fn owner(&self) -> &creamui_reactive::Owner {
        &self.owner
    }

    pub fn root(&self) -> RuntimeNodeId {
        self.root
    }

    pub fn dispose(&self) {
        self.owner.dispose();
    }
}

impl Drop for MountedView {
    fn drop(&mut self) {
        self.dispose();
    }
}

/// Anything a component can return that a [`MountCx`] knows how to mount.
pub trait IntoView {
    fn into_view(self) -> View;
}

impl IntoView for RuntimeNodeId {
    fn into_view(self) -> View {
        View::Mounted(self)
    }
}

impl IntoView for BoxedWidget {
    fn into_view(self) -> View {
        View::Legacy(self)
    }
}

impl<W: crate::Widget + 'static> IntoView for Box<W> {
    fn into_view(self) -> View {
        View::Legacy(self)
    }
}

impl IntoView for View {
    fn into_view(self) -> View {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{NodeKind, Runtime, SharedRuntime};
    use crate::{Painter, Rect, Widget};
    use creamui_reactive::Owner;

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
    fn mounted_view_returns_its_id_unchanged() {
        let (_runtime, _root, cx) = root_cx();
        let node = cx.container();
        assert_eq!(node.into_view().mount(&cx), node);
    }

    #[test]
    fn deferred_view_waits_for_mount_and_uses_the_supplied_scope() {
        let runs = std::rc::Rc::new(std::cell::Cell::new(0));
        let view = View::new({
            let runs = runs.clone();
            move |cx| {
                runs.set(runs.get() + 1);
                cx.text("deferred")
            }
        });
        assert_eq!(runs.get(), 0);
        let (runtime, root, cx) = root_cx();
        let node = view.mount(&cx);
        assert_eq!(runs.get(), 1);
        assert_eq!(
            runtime.with(|runtime| runtime.get(root).unwrap().children.as_slice().to_vec()),
            vec![node]
        );
        runtime.with(Runtime::check_invariants);
    }

    #[test]
    fn deferred_and_widget_views_respect_detached_mounting() {
        let (runtime, root, cx) = root_cx();
        let cx = cx.detached();
        let direct = View::new(|cx| cx.text("detached")).mount(&cx);
        let widget = Box::new(BlankWidget).into_view().mount(&cx);
        assert!(runtime.with(|runtime| runtime.get(root).unwrap().children.is_empty()));
        assert!(runtime.with(|runtime| runtime.get(direct).unwrap().parent.is_none()));
        assert!(runtime.with(|runtime| runtime.get(widget).unwrap().parent.is_none()));
        runtime.with(Runtime::check_invariants);
    }

    #[test]
    fn mounted_view_drop_disposes_bindings_and_releases_its_entire_runtime() {
        let signal = creamui_reactive::Signal::new(0u32);
        let runs = std::rc::Rc::new(std::cell::Cell::new(0));
        let cleaned = std::rc::Rc::new(std::cell::Cell::new(0));
        let mounted = MountedView::new(View::new({
            let signal = signal.clone();
            let runs = runs.clone();
            let cleaned = cleaned.clone();
            move |cx| {
                let node = cx.text("");
                cx.bind(move |tx| {
                    runs.set(runs.get() + 1);
                    tx.apply(super::super::Mutation::SetText {
                        node,
                        text: signal.get().to_string().into(),
                    });
                });
                cx.on_cleanup(move || cleaned.set(cleaned.get() + 1));
                node
            }
        }));
        let runtime = mounted.runtime().clone();
        let scope = mounted.owner().clone();
        assert_eq!(runtime.with(Runtime::len), 2);
        signal.set(1);
        assert_eq!(runs.get(), 2);
        drop(mounted);
        assert_eq!(cleaned.get(), 1);
        assert!(scope.is_disposed());
        assert!(runtime.with(Runtime::is_empty));
        assert_eq!(runtime.with(Runtime::root), None);
        signal.set(2);
        assert_eq!(runs.get(), 2);
        scope.dispose();
        assert_eq!(cleaned.get(), 1);
        runtime.with(Runtime::check_invariants);
    }

    #[test]
    fn disposing_a_mounted_view_notifies_observers_when_the_root_is_removed() {
        let mounted = MountedView::new(View::new(|cx| cx.container()));
        let runtime = mounted.runtime().clone();
        let notifications = std::rc::Rc::new(std::cell::Cell::new(0));
        let listener: std::rc::Rc<dyn Fn()> = {
            let runtime = runtime.clone();
            let notifications = notifications.clone();
            std::rc::Rc::new(move || {
                assert!(runtime.with(Runtime::is_empty));
                notifications.set(notifications.get() + 1);
            })
        };
        runtime.subscribe(&listener);
        mounted.dispose();
        assert_eq!(notifications.get(), 1);
        mounted.dispose();
        assert_eq!(notifications.get(), 1);
        runtime.with(Runtime::check_invariants);
    }

    #[test]
    fn removing_detached_nodes_notifies_observers_without_a_registered_root() {
        let runtime = SharedRuntime::new(Runtime::new());
        let node = runtime.transaction(|tx| tx.create_node(NodeKind::Container));
        let notifications = std::rc::Rc::new(std::cell::Cell::new(0));
        let listener: std::rc::Rc<dyn Fn()> = {
            let notifications = notifications.clone();
            std::rc::Rc::new(move || notifications.set(notifications.get() + 1))
        };
        runtime.subscribe(&listener);
        runtime.transaction(|tx| tx.remove_subtree(node));
        assert_eq!(notifications.get(), 1);
        assert!(runtime.with(Runtime::is_empty));
        runtime.with(Runtime::check_invariants);
    }

    #[test]
    fn mounted_view_keeps_dynamic_root_regions_under_a_stable_host() {
        let open = creamui_reactive::Signal::new(false);
        let mounted = MountedView::new(View::new({
            let open = open.clone();
            move |cx| cx.branch(open, |cx| cx.text("open"))
        }));
        let root = mounted.root();
        assert_eq!(mounted.runtime().with(Runtime::len), 2);
        open.set(true);
        assert_eq!(mounted.runtime().with(Runtime::len), 3);
        open.set(false);
        assert_eq!(mounted.runtime().with(Runtime::len), 2);
        assert_eq!(mounted.runtime().with(Runtime::root), Some(root));
        mounted.runtime().with(Runtime::check_invariants);
    }

    #[test]
    fn failed_mount_releases_partial_nodes_and_bindings() {
        let runtime = SharedRuntime::new(Runtime::new());
        let signal = creamui_reactive::Signal::new(0u32);
        let runs = std::rc::Rc::new(std::cell::Cell::new(0));
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe({
            let runtime = runtime.clone();
            let signal = signal.clone();
            let runs = runs.clone();
            move || {
                MountedView::mount_into(
                    runtime,
                    View::new(move |cx| {
                        let node = cx.text("");
                        cx.bind(move |_| {
                            signal.get();
                            runs.set(runs.get() + 1);
                        });
                        let _ = node;
                        panic!("mount failed");
                    }),
                );
            }
        }));
        assert!(failed.is_err());
        assert!(runtime.with(Runtime::is_empty));
        signal.set(1);
        assert_eq!(runs.get(), 1);
        runtime.with(Runtime::check_invariants);
    }

    struct BlankWidget;
    impl Widget for BlankWidget {
        fn style(&self) -> crate::Style {
            crate::Style::default()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
    }

    #[test]
    fn legacy_view_mounts_the_widget_under_the_cx_parent() {
        let (runtime, root, cx) = root_cx();
        let widget: BoxedWidget = Box::new(BlankWidget);

        let mounted = widget.into_view().mount(&cx);

        assert_eq!(
            runtime.with(|r| r.get(root).unwrap().children.as_slice().to_vec()),
            vec![mounted]
        );
    }
}
