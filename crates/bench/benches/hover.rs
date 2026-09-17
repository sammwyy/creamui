//! Cost of a hover transition: re-record the tree, diff, and rasterize only
//! the rows whose appearance changed.

use creamui_bench::painter::FramePipeline;
use creamui_bench::scenes;
use creamui_core::{Point, Renderer, Size};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

const VIEWPORT: Size = Size {
    width: 1920.0,
    height: 1080.0,
};

fn bench_hover_transition(c: &mut Criterion) {
    let mut group = c.benchmark_group("hover/frame_after_hover_change");
    for &count in &[1_000usize, 10_000] {
        let mut renderer = Renderer::new();
        let mut frames = FramePipeline::new(1920, 1080, 1.0);
        renderer.update(scenes::hover_list(count), VIEWPORT);
        frames.frame(&renderer);
        let mut row = 0;
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                row = (row + 1) % 40;
                frames.set_pointer(Some(Point {
                    x: 10.0,
                    y: row as f32 * 24.0 + 4.0,
                }));
                frames.frame(&renderer)
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_hover_transition);
criterion_main!(benches);
