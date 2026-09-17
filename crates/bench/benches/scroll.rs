//! Cost of a frame whose output did not change (e.g. a redundant redraw):
//! the tree is re-recorded and diffed, but nothing is rasterized.

use creamui_bench::painter::FramePipeline;
use creamui_bench::scenes;
use creamui_core::{Renderer, Size};
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

criterion_group!(benches, bench_unchanged_frame);
criterion_main!(benches);
