//! Cost of the persistent runtime tree (`creamui_core::runtime`), compared
//! against `tree_update`'s reconcile-based numbers for the same shapes.

use creamui_bench::scenes;
use creamui_core::runtime::{create_binding, Mutation, NodeKind, Runtime, SharedRuntime};
use creamui_core::PaintStyle;
use creamui_reactive::{Owner, Signal};
use creamui_theme::Color;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_mount_legacy_widget(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime/mount_legacy_widget");
    for &count in &[1_000usize, 10_000, 50_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let mut runtime = Runtime::new();
                let mut tx = runtime.transaction();
                creamui_core::runtime::mount_legacy_widget(&mut tx, scenes::wide_tree(count), None);
            });
        });
    }
    group.finish();
}

/// Mutates exactly one already-mounted leaf directly, bypassing widget
/// rebuild/reconcile entirely — the cost this benchmark should show as
/// independent of tree size, unlike `tree_update`'s reconcile-based
/// `unchanged_rerender`/single-leaf cases.
fn bench_direct_leaf_mutation(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime/direct_single_leaf_mutation");
    for &count in &[1_000usize, 10_000, 50_000] {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let root =
            creamui_core::runtime::mount_legacy_widget(&mut tx, scenes::wide_tree(count), None);
        drop(tx);
        let leaf = *runtime
            .get(root)
            .expect("root exists")
            .children
            .as_slice()
            .first()
            .expect("wide_tree has at least one leaf");

        let mut tick = 0u8;
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                tick = tick.wrapping_add(1);
                let mut tx = runtime.transaction();
                tx.apply(Mutation::SetPaintStyle {
                    node: leaf,
                    style: PaintStyle {
                        background: Some(Color::rgb(200, 40, tick).into()),
                        ..Default::default()
                    },
                });
            });
        });
    }
    group.finish();
}

/// Same shape as `direct_single_leaf_mutation`, but through the actual
/// Phase 3 path: a `Signal` write runs a `create_binding` effect that
/// mutates one leaf, rather than a benchmark calling `RuntimeTransaction`
/// directly. Proves the reactive wiring itself doesn't reintroduce a
/// tree-size-dependent cost.
fn bench_signal_driven_leaf_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime/signal_driven_single_leaf_update");
    for &count in &[1_000usize, 10_000, 50_000] {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let root =
            creamui_core::runtime::mount_legacy_widget(&mut tx, scenes::wide_tree(count), None);
        drop(tx);
        let leaf = *runtime
            .get(root)
            .expect("root exists")
            .children
            .as_slice()
            .first()
            .expect("wide_tree has at least one leaf");

        let runtime = SharedRuntime::new(runtime);
        let color = Signal::new(0u8);
        let owner = Owner::new();
        create_binding(&owner, runtime.clone(), {
            let color = color.clone();
            move |tx| {
                tx.apply(Mutation::SetPaintStyle {
                    node: leaf,
                    style: PaintStyle {
                        background: Some(Color::rgb(200, 40, color.get()).into()),
                        ..Default::default()
                    },
                });
            }
        });

        let mut tick = 0u8;
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                tick = tick.wrapping_add(1);
                color.set(tick);
            });
        });
        owner.dispose();
    }
    group.finish();
}

/// `compute_layout` cost after changing one leaf's layout style, as a
/// function of unrelated tree size — should stay flat, since `taffy`
/// caches unaffected subtrees internally and `Runtime` only calls
/// `compute_layout` when something is actually dirty.
fn bench_compute_layout_after_single_leaf_style_change(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime/compute_layout_after_single_leaf_style_change");
    for &count in &[1_000usize, 10_000, 50_000] {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let root =
            creamui_core::runtime::mount_legacy_widget(&mut tx, scenes::wide_tree(count), None);
        drop(tx);
        runtime.set_root(Some(root));
        runtime.compute_layout(creamui_core::Size {
            width: 1920.0,
            height: 1080.0,
        });
        let leaf = *runtime
            .get(root)
            .expect("root exists")
            .children
            .as_slice()
            .first()
            .expect("wide_tree has at least one leaf");

        let mut tick = 0u16;
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                tick = tick.wrapping_add(1);
                let mut tx = runtime.transaction();
                tx.apply(Mutation::SetLayoutStyle {
                    node: leaf,
                    style: creamui_core::layout::Style {
                        size: creamui_core::layout::Size {
                            width: creamui_core::layout::Dimension::Length(
                                16.0 + (tick % 8) as f32,
                            ),
                            height: creamui_core::layout::Dimension::Length(16.0),
                        },
                        ..Default::default()
                    },
                });
                drop(tx);
                runtime.compute_layout(creamui_core::Size {
                    width: 1920.0,
                    height: 1080.0,
                });
            });
        });
    }
    group.finish();
}

/// `rebuild_hit_test` cost as a function of total tree size, with a single
/// interactive leaf — walks every node to find interactive ones, so this
/// is expected to scale with tree size rather than interactive-node count.
fn bench_rebuild_hit_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime/rebuild_hit_test");
    for &count in &[1_000usize, 10_000, 50_000] {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let root =
            creamui_core::runtime::mount_legacy_widget(&mut tx, scenes::wide_tree(count), None);
        let leaf = *tx
            .touched()
            .last()
            .expect("mount_legacy_widget touches every node it creates");
        drop(tx);
        runtime.set_root(Some(root));

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                // `SetEventHandlers` always marks HIT_TEST dirty (handlers
                // aren't diffable), so this re-dirties the list each
                // iteration instead of measuring an already-fresh no-op.
                let mut tx = runtime.transaction();
                tx.apply(Mutation::SetEventHandlers {
                    node: leaf,
                    handlers: creamui_core::runtime::EventState {
                        on_click: Some(std::rc::Rc::new(|| {})),
                        ..Default::default()
                    },
                });
                drop(tx);
                runtime.rebuild_hit_test();
            });
        });
    }
    group.finish();
}

/// `rebuild_paint` cost after changing one leaf's background, as a
/// function of unrelated tree size — the paint queue is targeted (not a
/// tree walk), so this should stay flat.
fn bench_rebuild_paint_after_single_leaf_paint_change(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime/rebuild_paint_after_single_leaf_paint_change");
    let colors = creamui_theme::ColorScheme::default();
    for &count in &[1_000usize, 10_000, 50_000] {
        let mut runtime = Runtime::new();
        let mut tx = runtime.transaction();
        let root =
            creamui_core::runtime::mount_legacy_widget(&mut tx, scenes::wide_tree(count), None);
        drop(tx);
        runtime.set_root(Some(root));
        runtime.compute_layout(creamui_core::Size {
            width: 1920.0,
            height: 1080.0,
        });
        runtime.rebuild_paint(&colors);
        let leaf = *runtime
            .get(root)
            .expect("root exists")
            .children
            .as_slice()
            .first()
            .expect("wide_tree has at least one leaf");

        let mut tick = 0u8;
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                tick = tick.wrapping_add(1);
                let mut tx = runtime.transaction();
                tx.apply(Mutation::SetPaintStyle {
                    node: leaf,
                    style: PaintStyle {
                        background: Some(Color::rgb(200, 40, tick).into()),
                        ..Default::default()
                    },
                });
                drop(tx);
                runtime.rebuild_paint(&colors);
            });
        });
    }
    group.finish();
}

fn bench_create_node(c: &mut Criterion) {
    c.bench_function("runtime/create_10000_nodes", |b| {
        b.iter(|| {
            let mut runtime = Runtime::new();
            let mut tx = runtime.transaction();
            let root = tx.create_node(NodeKind::Container);
            for _ in 0..10_000 {
                let child = tx.create_node(NodeKind::Container);
                tx.insert_child(root, child, None);
            }
        });
    });
}

criterion_group!(
    benches,
    bench_mount_legacy_widget,
    bench_direct_leaf_mutation,
    bench_signal_driven_leaf_update,
    bench_compute_layout_after_single_leaf_style_change,
    bench_rebuild_hit_test,
    bench_rebuild_paint_after_single_leaf_paint_change,
    bench_create_node
);
criterion_main!(benches);
