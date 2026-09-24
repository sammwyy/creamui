//! Cost of a frame whose output did not change (e.g. a redundant redraw):
//! the tree is re-recorded and diffed, but nothing is rasterized; and of a
//! frame that scrolls a long list by one wheel step on the CPU rasterizer.

use creamui_bench::painter::FramePipeline;
use creamui_bench::scenes;
use creamui_core::{Renderer, Size};
use creamui_widgets::ScrollController;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

const VIEWPORT: Size = Size {
    width: 800.0,
    height: 1000.0,
};

fn bench_unchanged_frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll/unchanged_frame");
    for &count in &[1_000usize, 10_000] {
        let mut renderer = Renderer::new();
        let mut frames = FramePipeline::new(800, 1000, 1.0);
        renderer.update(scenes::chat(count), VIEWPORT);
        frames.frame(&renderer);
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| frames.frame(&renderer));
        });
    }
    group.finish();
}

fn bench_offset_change(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll/offset_change");
    for &count in &[1_000usize, 10_000] {
        let controller = ScrollController::new(400.0);
        let mut renderer = Renderer::new();
        let mut frames = FramePipeline::new(800, 1000, 1.0);
        renderer.update(
            scenes::scrolled_chat(count, 800.0, 1000.0, controller.clone()),
            VIEWPORT,
        );
        frames.frame(&renderer);
        let mut down = true;
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                controller.set(if down { 440.0 } else { 400.0 });
                down = !down;
                frames.frame(&renderer)
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_unchanged_frame, bench_offset_change);
criterion_main!(benches);
