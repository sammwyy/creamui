//! Real CPU raster cost, via `tiny-skia`.

use creamui_bench::painter::raster_painter;
use creamui_bench::scenes;
use creamui_core::{Renderer, Size};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

const VIEWPORT: Size = Size {
    width: 1920.0,
    height: 1080.0,
};

fn bench_initial_raster(c: &mut Criterion) {
    let mut group = c.benchmark_group("paint/initial_raster");
    for &count in &[1_000usize, 10_000, 50_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let mut renderer = Renderer::new();
                let mut painter = raster_painter(1920, 1080);
                renderer.render(scenes::wide_tree(count), VIEWPORT, &mut painter);
            });
        });
    }
    group.finish();
}

fn bench_dashboard_raster(c: &mut Criterion) {
    c.bench_function("paint/dashboard_initial_raster", |b| {
        b.iter(|| {
            let mut renderer = Renderer::new();
            let mut painter = raster_painter(1440, 900);
            renderer.render(
                scenes::dashboard(),
                Size {
                    width: 1440.0,
                    height: 900.0,
                },
                &mut painter,
            );
        });
    });
}

/// 5,000 static leaves plus a varying number of animated leaves — measures
/// [`Renderer::repaint_animated`]'s cost once those leaves are promoted to
/// their own layer, isolated from the static subtree it must prune.
fn bench_animated_cards_repaint(c: &mut Criterion) {
    let mut group = c.benchmark_group("paint/animated_cards_repaint_animated");
    for &animated in &[1usize, 10, 100] {
        let mut renderer = Renderer::new();
        let mut painter = raster_painter(1920, 1080);
        // Two full renders so the animated leaves' streak crosses the
        // layer-promotion threshold before the benchmark loop starts.
        renderer.render(
            scenes::animated_cards(5_000, animated),
            VIEWPORT,
            &mut painter,
        );
        renderer.render(
            scenes::animated_cards(5_000, animated),
            VIEWPORT,
            &mut painter,
        );
        group.bench_with_input(BenchmarkId::from_parameter(animated), &animated, |b, _| {
            b.iter(|| {
                renderer.repaint_animated(&mut painter, None, false);
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_initial_raster,
    bench_dashboard_raster,
    bench_animated_cards_repaint
);
criterion_main!(benches);
