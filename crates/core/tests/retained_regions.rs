use creamui_core::runtime::{
    EventState, MountCx, Mutation, NodeKind, Runtime, RuntimeNodeId, SharedRuntime,
};
use creamui_core::{Size, Style};
use creamui_reactive::{provide_context, use_context, with_context_scope, Owner, Signal};
use std::cell::Cell;
use std::rc::Rc;

fn context() -> (SharedRuntime, RuntimeNodeId, MountCx) {
    let mut runtime = Runtime::new();
    let root = runtime.transaction().create_node(NodeKind::Container);
    runtime.set_root(Some(root));
    let runtime = SharedRuntime::new(runtime);
    let cx = MountCx::new(runtime.clone(), Owner::new(), root);
    (runtime, root, cx)
}

fn content(runtime: &SharedRuntime, root: RuntimeNodeId) -> Vec<RuntimeNodeId> {
    runtime.with(|runtime| {
        runtime
            .get(root)
            .unwrap()
            .children
            .as_slice()
            .iter()
            .copied()
            .filter(|&node| {
                runtime.get(node).unwrap().layout_style.display
                    != creamui_core::layout::Display::None
            })
            .collect()
    })
}

fn fixed_text(cx: &MountCx, text: impl Into<Rc<str>>) -> RuntimeNodeId {
    let node = cx.text(text);
    cx.runtime().transaction(|tx| {
        tx.apply(Mutation::SetLayoutStyle {
            node,
            style: Style::new().width(10.0).height(10.0).layout,
        })
    });
    node
}

#[test]
fn independent_regions_preserve_static_siblings_and_flex_spacing() {
    let (runtime, root, cx) = context();
    runtime.transaction(|tx| {
        tx.apply(Mutation::SetLayoutStyle {
            node: root,
            style: Style::new().width(200.0).height(20.0).gap(2.0).layout,
        })
    });
    let prefix = fixed_text(&cx, "prefix");
    let a = Signal::new(vec![1u32, 2]);
    cx.keyed(
        a.clone(),
        |id| *id,
        |cx, id| fixed_text(cx, id.get().to_string()),
    );
    let open = Signal::new(false);
    cx.branch(open.clone(), |cx| fixed_text(cx, "branch"));
    let b = Signal::new(vec![3u32]);
    cx.keyed(
        b.clone(),
        |id| *id,
        |cx, id| fixed_text(cx, id.get().to_string()),
    );
    let suffix = fixed_text(&cx, "suffix");
    let original = content(&runtime, root);
    assert_eq!(original.len(), 5);
    a.set(vec![2, 1]);
    open.set(true);
    b.set(vec![4, 3]);
    let next = content(&runtime, root);
    assert_eq!(&next[..3], &[prefix, original[2], original[1]]);
    assert_eq!(&next[5..], &[original[3], suffix]);
    runtime.with_mut(|runtime| {
        runtime.check_invariants();
        runtime.compute_layout(Size {
            width: 200.0,
            height: 20.0,
        });
        for (index, id) in next.iter().enumerate() {
            assert_eq!(runtime.get(*id).unwrap().layout.rect.x, index as f32 * 12.0);
        }
    });
    a.set(vec![]);
    open.set(false);
    b.set(vec![3]);
    assert_eq!(content(&runtime, root), vec![prefix, original[3], suffix]);
    assert!(
        runtime.with(|runtime| runtime.get(next[3]).is_none() && runtime.get(next[4]).is_none())
    );
    cx.owner().dispose();
    assert_eq!(content(&runtime, root), vec![prefix, suffix]);
    runtime.with(Runtime::check_invariants);
}

#[test]
fn surviving_keys_update_values_and_events_without_remounting_or_rebinding_equal_items() {
    let (runtime, root, cx) = context();
    let items = Signal::new(vec![(1u32, "one".to_owned()), (2, "two".to_owned())]);
    let mounts = Rc::new(Cell::new(0));
    let bindings = Rc::new(Cell::new(0));
    let clicked = Rc::new(Cell::new(0));
    cx.keyed(items.clone(), |item| item.0, {
        let (mounts, bindings, clicked) = (mounts.clone(), bindings.clone(), clicked.clone());
        move |cx, item| {
            mounts.set(mounts.get() + 1);
            let node = cx.text("");
            cx.bind({
                let item = item.clone();
                let bindings = bindings.clone();
                move |tx| {
                    bindings.set(bindings.get() + 1);
                    tx.apply(Mutation::SetText {
                        node,
                        text: item.get().1.into(),
                    });
                }
            });
            cx.set_events(
                node,
                EventState {
                    focusable: true,
                    on_click: Some({
                        let clicked = clicked.clone();
                        Rc::new(move || clicked.set(item.get().1.len()))
                    }),
                    ..Default::default()
                },
            );
            node
        }
    });
    let original = content(&runtime, root);
    items.set(vec![(2, "two".to_owned()), (1, "updated".to_owned())]);
    assert_eq!(content(&runtime, root), vec![original[1], original[0]]);
    assert_eq!(mounts.get(), 2);
    assert_eq!(bindings.get(), 3);
    runtime.with(|runtime| {
        let node = runtime.get(original[0]).unwrap();
        assert!(matches!(&node.kind, NodeKind::Text(text) if &*text.text == "updated"));
        (node.events.on_click.as_ref().unwrap())();
    });
    assert_eq!(clicked.get(), 7);
    items.set(vec![(1, "updated".to_owned()), (2, "two".to_owned())]);
    assert_eq!(bindings.get(), 3);
    cx.owner().dispose();
    items.set(vec![(3, "later".to_owned())]);
    assert!(content(&runtime, root).is_empty());
    assert_eq!(mounts.get(), 2);
    runtime.with(Runtime::check_invariants);
}

#[test]
fn derived_switches_isolate_content_reads_and_keep_the_mounting_context() {
    let (runtime, root, cx) = context();
    let condition = Signal::new(0u32);
    let value = Signal::new(10u32);
    let mounts = Rc::new(Cell::new(0));
    with_context_scope(|| {
        provide_context("scoped");
        cx.switch(
            {
                let condition = condition.clone();
                move || condition.get() > 0
            },
            {
                let value = value.clone();
                let mounts = mounts.clone();
                move |cx, active| {
                    mounts.set(mounts.get() + 1);
                    assert_eq!(use_context::<&'static str>(), "scoped");
                    let node = cx.text(if active { "active" } else { "inactive" });
                    cx.bind({
                        let value = value.clone();
                        move |tx| {
                            tx.apply(Mutation::SetText {
                                node,
                                text: value.get().to_string().into(),
                            });
                        }
                    });
                    node
                }
            },
        );
    });
    let initial = content(&runtime, root)[0];
    value.set(11);
    assert_eq!(mounts.get(), 1);
    assert_eq!(content(&runtime, root), vec![initial]);
    condition.set(1);
    let next = content(&runtime, root)[0];
    assert_ne!(next, initial);
    condition.set(2);
    value.set(12);
    assert_eq!(mounts.get(), 2);
    assert_eq!(content(&runtime, root), vec![next]);
    assert!(runtime.with(|runtime| runtime.get(initial).is_none()));
    cx.owner().dispose();
    assert!(content(&runtime, root).is_empty());
}

#[test]
fn derived_lists_retain_matching_keys_when_the_source_filter_changes() {
    let (runtime, root, cx) = context();
    let items = Signal::new(vec![1u32, 2, 3]);
    let even = Signal::new(false);
    cx.keyed_with(
        {
            let items = items.clone();
            let even = even.clone();
            move || {
                let even = even.get();
                items
                    .get()
                    .into_iter()
                    .filter(|item| !even || item % 2 == 0)
                    .collect()
            }
        },
        |id| *id,
        |cx, id| cx.text(id.get().to_string()),
    );
    let original = content(&runtime, root);
    even.set(true);
    assert_eq!(content(&runtime, root), vec![original[1]]);
    items.set(vec![4, 2]);
    let next = content(&runtime, root);
    assert_eq!(next[1], original[1]);
    runtime.with(Runtime::check_invariants);
}

#[test]
fn duplicate_keys_reject_an_update_without_changing_the_mounted_tree() {
    let (runtime, root, cx) = context();
    let items = Signal::new(vec![1u32, 2]);
    cx.keyed(
        items.clone(),
        |id| *id,
        |cx, id| cx.text(id.get().to_string()),
    );
    let original = content(&runtime, root);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| items.set(vec![1, 1])));
    assert!(result.is_err());
    assert_eq!(content(&runtime, root), original);
    items.set(vec![2, 1]);
    assert_eq!(content(&runtime, root), vec![original[1], original[0]]);
    runtime.with(Runtime::check_invariants);
}

#[test]
fn region_mounting_does_not_keep_its_parent_owner_alive() {
    let (runtime, root, cx) = context();
    let weak = cx.owner().downgrade();
    cx.branch(Signal::new(true), |cx| cx.text("branch"));
    cx.keyed(
        Signal::new(vec![1u32]),
        |id| *id,
        |cx, id| cx.text(id.get().to_string()),
    );
    drop(cx);
    assert!(weak.upgrade().is_none());
    runtime.with(Runtime::check_invariants);
    assert_eq!(content(&runtime, root).len(), 2);
}

#[cfg(feature = "perf-metrics")]
#[test]
fn wide_keyed_regions_batch_mount_reorder_and_removal() {
    let (_, _, cx) = context();
    let items = Signal::new((0..4096u32).collect::<Vec<_>>());
    creamui_core::metrics::reset_frame_metrics();
    cx.keyed(
        items.clone(),
        |id| *id,
        |cx, id| cx.text(id.get().to_string()),
    );
    assert_eq!(
        creamui_core::metrics::frame_metrics().taffy_children_writes,
        2
    );
    creamui_core::metrics::reset_frame_metrics();
    items.update(|items| items.reverse());
    assert_eq!(
        creamui_core::metrics::frame_metrics().taffy_children_writes,
        1
    );
    creamui_core::metrics::reset_frame_metrics();
    items.set(vec![]);
    assert_eq!(
        creamui_core::metrics::frame_metrics().taffy_children_writes,
        1
    );
}

#[test]
fn branch_cleanup_can_change_its_condition_without_reentering_reconciliation() {
    let (runtime, root, cx) = context();
    let condition = Signal::new(true);
    cx.branch(condition.clone(), {
        let condition = condition.clone();
        move |cx| {
            cx.owner().on_cleanup({
                let condition = condition.clone();
                move || condition.set(true)
            });
            cx.text("active")
        }
    });
    let previous = content(&runtime, root)[0];
    condition.set(false);
    let next = content(&runtime, root)[0];
    assert_ne!(next, previous);
    assert!(condition.peek());
    assert!(runtime.with(|runtime| runtime.get(previous).is_none()));
    cx.owner().dispose();
    assert!(content(&runtime, root).is_empty());
    runtime.with(Runtime::check_invariants);
}

#[test]
fn keyed_cleanup_can_replace_its_source_without_reentering_reconciliation() {
    let (runtime, root, cx) = context();
    let items = Signal::new(vec![1u32]);
    cx.keyed(items.clone(), |item| *item, {
        let items = items.clone();
        move |cx, item| {
            if item.get() == 1 {
                cx.owner().on_cleanup({
                    let items = items.clone();
                    move || items.set(vec![2])
                });
            }
            cx.text(item.get().to_string())
        }
    });
    let previous = content(&runtime, root)[0];
    items.set(vec![]);
    let next = content(&runtime, root)[0];
    assert_ne!(next, previous);
    assert_eq!(items.peek(), vec![2]);
    assert!(runtime.with(|runtime| runtime.get(previous).is_none()));
    cx.owner().dispose();
    assert!(content(&runtime, root).is_empty());
    runtime.with(Runtime::check_invariants);
}

#[test]
fn branch_mount_can_dispose_its_parent_without_leaving_detached_nodes() {
    let (runtime, root, cx) = context();
    cx.branch(Signal::new(true), {
        let parent = cx.owner().clone();
        move |cx| {
            let node = cx.text("disposed");
            parent.dispose();
            node
        }
    });
    assert!(runtime.with(|runtime| runtime.get(root).unwrap().children.is_empty()));
    assert_eq!(runtime.with(|runtime| runtime.len()), 1);
    runtime.with(Runtime::check_invariants);
}

#[test]
fn keyed_mount_can_dispose_its_parent_without_leaving_detached_nodes() {
    let (runtime, root, cx) = context();
    let mounts = Rc::new(Cell::new(0));
    cx.keyed(Signal::new(vec![1u32, 2, 3]), |item| *item, {
        let parent = cx.owner().clone();
        let mounts = mounts.clone();
        move |cx, _item| {
            let node = cx.text("disposed");
            mounts.set(mounts.get() + 1);
            parent.dispose();
            node
        }
    });
    assert_eq!(mounts.get(), 1);
    assert!(runtime.with(|runtime| runtime.get(root).unwrap().children.is_empty()));
    assert_eq!(runtime.with(|runtime| runtime.len()), 1);
    runtime.with(Runtime::check_invariants);
}

#[test]
fn overlapping_deep_subtrees_are_removed_without_recursion_or_affecting_siblings() {
    let (runtime, root, cx) = context();
    let retained = cx.text("retained");
    let nodes = runtime.transaction(|tx| {
        let nodes: Vec<_> = (0..10_000)
            .map(|_| tx.create_node(NodeKind::Container))
            .collect();
        for pair in nodes.windows(2).rev() {
            tx.insert_child(pair[0], pair[1], None);
        }
        tx.insert_child(root, nodes[0], None);
        nodes
    });
    #[cfg(feature = "perf-metrics")]
    creamui_core::metrics::reset_frame_metrics();
    runtime.transaction(|tx| tx.remove_subtrees(&nodes));
    #[cfg(feature = "perf-metrics")]
    assert_eq!(
        creamui_core::metrics::frame_metrics().taffy_children_writes,
        1
    );
    assert_eq!(content(&runtime, root), vec![retained]);
    assert_eq!(runtime.with(|runtime| runtime.len()), 2);
    runtime.with(Runtime::check_invariants);
}
