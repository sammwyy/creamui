use super::mount::mount_legacy_widget;
use super::mount_cx::MountCx;
use super::node::RuntimeNodeId;
use crate::BoxedWidget;

/// A component's mount result: either a node already mounted into the
/// runtime tree directly (the target path), or a legacy `Widget` subtree
/// translated through [`mount_legacy_widget`] (the compatibility path).
/// Lets existing custom components keep returning a `BoxedWidget` while
/// migrated ones return an already-mounted [`RuntimeNodeId`].
pub enum View {
    Mounted(RuntimeNodeId),
    Legacy(BoxedWidget),
}

impl View {
    pub fn mount(self, cx: &MountCx) -> RuntimeNodeId {
        match self {
            View::Mounted(id) => id,
            View::Legacy(widget) => cx
                .runtime()
                .transaction(|tx| mount_legacy_widget(tx, widget, Some(cx.parent()))),
        }
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
