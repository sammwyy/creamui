//! Cost of the persistent runtime tree (`creamui_core::runtime`), compared
//! against `tree_update`'s reconcile-based numbers for the same shapes.

use creamui_bench::scenes;
use creamui_core::runtime::{Mutation, NodeKind, Runtime};
use creamui_core::PaintStyle;
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
    bench_create_node
);
criterion_main!(benches);
