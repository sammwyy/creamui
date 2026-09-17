//! Full frame cost through the real pipeline: record the retained tree,
//! diff it against the previous frame and rasterize the damage on the CPU.

use creamui_bench::painter::FramePipeline;
use creamui_bench::scenes;
use creamui_core::{Renderer, Size};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

const VIEWPORT: Size = Size {
    width: 1920.0,
    height: 1080.0,
};

fn bench_initial_frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("paint/initial_frame");
    for &count in &[1_000usize, 10_000, 50_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let mut renderer = Renderer::new();
                let mut frames = FramePipeline::new(1920, 1080, 1.0);
                renderer.update(scenes::wide_tree(count), VIEWPORT);
                frames.frame(&renderer);
            });
        });
    }
    group.finish();
}

fn bench_dashboard_frame(c: &mut Criterion) {
    let viewport = Size {
        width: 1440.0,
        height: 900.0,
    };
    c.bench_function("paint/dashboard_initial_frame", |b| {
        b.iter(|| {
            let mut renderer = Renderer::new();
            let mut frames = FramePipeline::new(1440, 900, 1.0);
            renderer.update(scenes::dashboard(), viewport);
            frames.frame(&renderer);
        });
    });
    c.bench_function("paint/dashboard_hidpi_initial_frame", |b| {
        b.iter(|| {
            let mut renderer = Renderer::new();
            let mut frames = FramePipeline::new(2880, 1800, 2.0);
            renderer.update(scenes::dashboard(), viewport);
            frames.frame(&renderer);
        });
    });
}

/// 5,000 static leaves plus animated leaves: every tick re-records the tree
/// and rasterizes only the animated leaves.
fn bench_animated_cards_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("paint/animated_cards_tick");
    for &animated in &[1usize, 10, 100] {
        let mut renderer = Renderer::new();
        let mut frames = FramePipeline::new(1920, 1080, 1.0);
        renderer.update(scenes::animated_cards(5_000, animated), VIEWPORT);
        frames.frame(&renderer);
        group.bench_with_input(BenchmarkId::from_parameter(animated), &animated, |b, _| {
            b.iter(|| frames.frame(&renderer));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_initial_frame,
    bench_dashboard_frame,
    bench_animated_cards_tick
);
criterion_main!(benches);
