//! Cost of a hover-triggered repaint. There is no node-local hover repaint
//! path: a hover transition repaints the whole retained tree via
//! [`Renderer::repaint_focused`], so this measures how that cost scales
//! with tree size.

use creamui_bench::painter::raster_painter;
use creamui_bench::scenes;
use creamui_core::{Renderer, Size};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

const VIEWPORT: Size = Size {
    width: 1920.0,
    height: 1080.0,
};

fn bench_hover_triggered_repaint(c: &mut Criterion) {
    let mut group = c.benchmark_group("hover/repaint_after_hover_change");
    for &count in &[1_000usize, 10_000, 50_000] {
        let mut renderer = Renderer::new();
        let mut painter = raster_painter(1920, 1080);
        renderer.render(scenes::wide_tree(count), VIEWPORT, &mut painter);
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                renderer.repaint_focused(&mut painter, None, false);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_hover_triggered_repaint);
criterion_main!(benches);
