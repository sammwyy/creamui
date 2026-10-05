use super::mutation::Mutation;
use super::node::{CustomNode, NodeKind, RuntimeNodeId};
use super::transaction::RuntimeTransaction;
use crate::BoxedWidget;

/// Translates a legacy `Widget` subtree into the runtime tree, once. This
/// is a one-shot adapter (not a retained diff against a previous mount):
/// the runtime tree becomes the only place its style/structure lives
/// afterward. Widgets without a primitive kind are retained for painting.
pub fn mount_legacy_widget(
    tx: &mut RuntimeTransaction,
    mut widget: BoxedWidget,
    parent: Option<RuntimeNodeId>,
) -> RuntimeNodeId {
    #[cfg(feature = "perf-metrics")]
    crate::metrics::record(|m| m.legacy_widgets_mounted += 1);

    let style = widget.style();
    let kind = widget.legacy_node_kind();
    let child_widgets = widget.children();
    let measure = widget.measure();
    let measure_fingerprint = widget.measure_fingerprint();
    let kind = match kind {
        Some(NodeKind::Custom(mut custom)) => {
            custom.widget = Some(std::rc::Rc::from(widget));
            NodeKind::Custom(custom)
        }
        Some(kind) => kind,
        None => NodeKind::Custom(CustomNode {
            widget: Some(std::rc::Rc::from(widget)),
            ..Default::default()
        }),
    };

    let id = tx.create_node(kind);
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
    if measure.is_some() {
        tx.apply(Mutation::SetMeasure {
            node: id,
            measure,
            fingerprint: measure_fingerprint,
        });
    }

    if let Some(parent) = parent {
        tx.insert_child(parent, id, None);
    }

    if !child_widgets.is_empty() {
        let children: Vec<_> = child_widgets
            .into_iter()
            .map(|child| mount_legacy_widget(tx, child, None))
            .collect();
        tx.reorder_children(id, &children);
    }

    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{PaintOp, PaintPrimitive, Runtime, TextNode};
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
    fn mounting_a_wide_subtree_batches_child_writes_and_preserves_order() {
        let widget: BoxedWidget = Box::new(Branch {
            width: 800.0,
            children: (0..256)
                .map(|index| {
                    Box::new(Branch {
                        width: index as f32 + 1.0,
                        children: vec![
                            Box::new(Branch {
                                width: 3.0,
                                children: vec![],
                            }),
                            Box::new(Branch {
                                width: 4.0,
                                children: vec![],
                            }),
                        ],
                    }) as BoxedWidget
                })
                .collect(),
        });
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let parent = tx.create_node(NodeKind::Container);
        #[cfg(feature = "perf-metrics")]
        crate::metrics::reset_frame_metrics();
        let root = mount_legacy_widget(&mut tx, widget, Some(parent));
        drop(tx);
        assert_eq!(runtime.get(parent).unwrap().children.as_slice(), &[root]);
        for (index, &row) in runtime
            .get(root)
            .unwrap()
            .children
            .as_slice()
            .iter()
            .enumerate()
        {
            let row = runtime.get(row).unwrap();
            assert_eq!(
                row.layout_style.size.width,
                taffy::style::Dimension::Length(index as f32 + 1.0)
            );
            assert_eq!(row.parent, Some(root));
            for (&child, width) in row.children.as_slice().iter().zip([3.0, 4.0]) {
                let child = runtime.get(child).unwrap();
                assert_eq!(child.parent, Some(row.id));
                assert_eq!(
                    child.layout_style.size.width,
                    taffy::style::Dimension::Length(width)
                );
            }
            assert_eq!(row.children.len(), 2);
        }
        assert_eq!(runtime.get(root).unwrap().children.len(), 256);
        assert_eq!(runtime.len(), 770);
        #[cfg(feature = "perf-metrics")]
        {
            let metrics = crate::metrics::frame_metrics();
            assert_eq!(metrics.taffy_children_writes, 258);
            assert_eq!(metrics.legacy_widgets_mounted, 769);
        }
        runtime.check_invariants();
    }

    #[test]
    fn a_widget_that_does_not_override_legacy_node_kind_mounts_as_custom() {
        let widget: BoxedWidget = Box::new(Branch {
            width: 1.0,
            children: vec![],
        });

        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let root = mount_legacy_widget(&mut tx, widget, None);
        drop(tx);

        assert!(matches!(
            runtime.get(root).unwrap().kind,
            NodeKind::Custom(_)
        ));
    }

    struct ContentWidget;

    impl Widget for ContentWidget {
        fn style(&self) -> crate::Style {
            let mut layout = taffy::style::Style::default();
            layout.size.width = taffy::style::Dimension::Length(40.0);
            layout.size.height = taffy::style::Dimension::Length(30.0);
            layout.padding = taffy::geometry::Rect {
                left: taffy::style::LengthPercentage::Length(5.0),
                right: taffy::style::LengthPercentage::Length(5.0),
                top: taffy::style::LengthPercentage::Length(5.0),
                bottom: taffy::style::LengthPercentage::Length(5.0),
            };
            layout.into()
        }

        fn paint(&self, _: &mut dyn Painter, _: Rect) {}

        fn paint_content(&self, painter: &mut dyn Painter, _: Rect, content: Rect) {
            painter.fill_rect(content, creamui_theme::Color::rgb(1, 2, 3), 0.0);
        }
    }

    #[test]
    fn custom_widget_paints_its_resolved_content_box_after_mount() {
        let mut runtime = Runtime::new();
        let node = mount_legacy_widget(&mut runtime.transaction(), Box::new(ContentWidget), None);
        runtime.set_root(Some(node));
        runtime.compute_layout(crate::Size {
            width: 100.0,
            height: 100.0,
        });
        runtime.rebuild_paint(&creamui_theme::ColorScheme::default());

        let fragment = runtime.get(node).unwrap().paint.fragment.as_ref().unwrap();
        let PaintOp::Primitive(PaintPrimitive::Quad(quad)) = &fragment.ops[0] else {
            panic!("custom widget should record a painted quad");
        };
        assert_eq!(
            quad.rect,
            Rect {
                x: 5.0,
                y: 5.0,
                width: 30.0,
                height: 20.0
            }
        );

        let mut style = runtime.get(node).unwrap().layout_style.clone();
        style.padding = taffy::geometry::Rect {
            left: taffy::style::LengthPercentage::Length(10.0),
            right: taffy::style::LengthPercentage::Length(10.0),
            top: taffy::style::LengthPercentage::Length(10.0),
            bottom: taffy::style::LengthPercentage::Length(10.0),
        };
        runtime
            .transaction()
            .apply(Mutation::SetLayoutStyle { node, style });
        runtime.compute_layout(crate::Size {
            width: 100.0,
            height: 100.0,
        });
        runtime.rebuild_paint(&creamui_theme::ColorScheme::default());
        let fragment = runtime.get(node).unwrap().paint.fragment.as_ref().unwrap();
        let PaintOp::Primitive(PaintPrimitive::Quad(quad)) = &fragment.ops[0] else {
            panic!("custom widget should repaint after padding changes");
        };
        assert_eq!(
            quad.rect,
            Rect {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 10.0
            }
        );
    }

    struct MeasuredWidget;

    impl Widget for MeasuredWidget {
        fn style(&self) -> crate::Style {
            crate::Style::default()
        }

        fn measure(&self) -> Option<crate::MeasureFn> {
            Some(Box::new(|_, _| taffy::geometry::Size {
                width: 11.0,
                height: 7.0,
            }))
        }

        fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
            painter.fill_rect(rect, creamui_theme::Color::rgb(1, 2, 3), 0.0);
        }
    }

    #[test]
    fn custom_widget_keeps_intrinsic_measurement_after_mount() {
        let mut runtime = Runtime::new();
        let node = mount_legacy_widget(&mut runtime.transaction(), Box::new(MeasuredWidget), None);
        runtime.set_root(Some(node));
        runtime.compute_layout(crate::Size {
            width: 100.0,
            height: 100.0,
        });
        assert_eq!(runtime.get(node).unwrap().layout.rect.width, 11.0);
        assert_eq!(runtime.get(node).unwrap().layout.rect.height, 7.0);
        runtime.rebuild_paint(&creamui_theme::ColorScheme::default());
        let fragment = runtime.get(node).unwrap().paint.fragment.as_ref().unwrap();
        let PaintOp::Primitive(PaintPrimitive::Quad(quad)) = &fragment.ops[0] else {
            panic!("measured widget should paint a quad");
        };
        assert_eq!(quad.rect.width, 11.0);
        assert_eq!(quad.rect.height, 7.0);
    }

    struct TextWidget {
        text: &'static str,
    }

    impl Widget for TextWidget {
        fn style(&self) -> crate::Style {
            crate::Style::new()
        }
        fn paint(&self, _painter: &mut dyn Painter, _rect: Rect) {}
        fn legacy_node_kind(&self) -> Option<NodeKind> {
            Some(NodeKind::Text(TextNode {
                text: self.text.into(),
            }))
        }
    }

    #[test]
    fn a_widget_overriding_legacy_node_kind_mounts_with_that_kind() {
        let widget: BoxedWidget = Box::new(TextWidget { text: "hello" });

        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let root = mount_legacy_widget(&mut tx, widget, None);
        drop(tx);

        match &runtime.get(root).unwrap().kind {
            NodeKind::Text(text) => assert_eq!(&*text.text, "hello"),
            other => panic!("expected a text node, got {other:?}"),
        }
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
