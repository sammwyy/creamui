//! Reconcile/layout cost as a function of tree shape and update kind.

use creamui_bench::painter::NoopPainter;
use creamui_bench::scenes;
use creamui_core::{Renderer, Size};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

const VIEWPORT: Size = Size {
    width: 1920.0,
    height: 1080.0,
};

fn bench_initial_mount(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_update/initial_mount");
    for &count in &[1_000usize, 10_000, 50_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let mut renderer = Renderer::new();
                let mut painter = NoopPainter;
                renderer.render(scenes::wide_tree(count), VIEWPORT, &mut painter);
            });
        });
    }
    group.finish();
}

fn bench_unchanged_rerender(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_update/unchanged_rerender");
    for &count in &[1_000usize, 10_000, 50_000] {
        let mut renderer = Renderer::new();
        let mut painter = NoopPainter;
        renderer.render(scenes::wide_tree(count), VIEWPORT, &mut painter);
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                renderer.render(scenes::wide_tree(count), VIEWPORT, &mut painter);
            });
        });
    }
    group.finish();
}

fn bench_deep_tree(c: &mut Criterion) {
    c.bench_function("tree_update/deep_tree_1000_unchanged_rerender", |b| {
        let mut renderer = Renderer::new();
        let mut painter = NoopPainter;
        renderer.render(scenes::deep_tree(1_000), VIEWPORT, &mut painter);
        b.iter(|| {
            renderer.render(scenes::deep_tree(1_000), VIEWPORT, &mut painter);
        });
    });
}

criterion_group!(
    benches,
    bench_initial_mount,
    bench_unchanged_rerender,
    bench_deep_tree
);
criterion_main!(benches);
