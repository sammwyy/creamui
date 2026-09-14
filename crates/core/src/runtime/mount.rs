use super::mutation::Mutation;
use super::node::{CustomNode, NodeKind, RuntimeNodeId};
use super::transaction::RuntimeTransaction;
use crate::BoxedWidget;

/// Translates a legacy `Widget` subtree into the runtime tree, once. This
/// is a one-shot adapter (not a retained diff against a previous mount):
/// the widget is fully consumed, and the runtime tree becomes the only
/// place its style/structure lives afterward.
pub fn mount_legacy_widget(
    tx: &mut RuntimeTransaction,
    mut widget: BoxedWidget,
    parent: Option<RuntimeNodeId>,
) -> RuntimeNodeId {
    #[cfg(feature = "perf-metrics")]
    crate::metrics::record(|m| m.legacy_widgets_mounted += 1);

    let style = widget.style();
    let child_widgets = widget.children();

    let id = tx.create_node(NodeKind::Custom(CustomNode::default()));
    tx.apply(Mutation::SetLayoutStyle {
        node: id,
        style: style.layout,
    });
    tx.apply(Mutation::SetPaintStyle {
        node: id,
        style: style.paint,
    });
    tx.apply(Mutation::SetTypographyStyle {
        node: id,
        style: style.typography,
    });

    if let Some(parent) = parent {
        tx.insert_child(parent, id, None);
    }

    for child_widget in child_widgets {
        mount_legacy_widget(tx, child_widget, Some(id));
    }

    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;
    use crate::{Painter, Rect, Widget};

    struct Branch {
        width: f32,
        children: Vec<BoxedWidget>,
    }

    impl Widget for Branch {
        fn style(&self) -> crate::Style {
            crate::Style::new().width(self.width)
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn children(&mut self) -> Vec<BoxedWidget> {
            std::mem::take(&mut self.children)
        }
    }

    #[test]
    fn mounts_a_widget_subtree_preserving_structure_and_style() {
        let widget: BoxedWidget = Box::new(Branch {
            width: 10.0,
            children: vec![
                Box::new(Branch {
                    width: 20.0,
                    children: vec![],
                }),
                Box::new(Branch {
                    width: 30.0,
                    children: vec![],
                }),
            ],
        });

        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let root = mount_legacy_widget(&mut tx, widget, None);
        drop(tx);
        runtime.set_root(Some(root));

        let root_node = runtime.get(root).unwrap();
        assert_eq!(
            root_node.layout_style.size.width,
            taffy::style::Dimension::Length(10.0)
        );
        assert_eq!(root_node.children.len(), 2);

        let widths: Vec<_> = root_node
            .children
            .as_slice()
            .iter()
            .map(
                |&id| match runtime.get(id).unwrap().layout_style.size.width {
                    taffy::style::Dimension::Length(w) => w,
                    other => panic!("unexpected dimension: {other:?}"),
                },
            )
            .collect();
        assert_eq!(widths, vec![20.0, 30.0]);

        runtime.check_invariants();
    }

    #[test]
    fn mounting_increments_the_legacy_mount_counter() {
        #[cfg(feature = "perf-metrics")]
        {
            let widget: BoxedWidget = Box::new(Branch {
                width: 1.0,
                children: vec![Box::new(Branch {
                    width: 2.0,
                    children: vec![],
                })],
            });
            crate::metrics::reset_frame_metrics();
            let mut runtime = Runtime::new();
            let mut tx = runtime.transaction();
            mount_legacy_widget(&mut tx, widget, None);
            assert_eq!(crate::metrics::frame_metrics().legacy_widgets_mounted, 2);
        }
    }
}
