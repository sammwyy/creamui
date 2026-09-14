//! Cost of a scroll-offset-triggered repaint. There is no transform-only
//! scroll path: a scroll offset change repaints the whole scrollable
//! subtree via [`Renderer::repaint_focused`], so this measures how that
//! cost scales with content size.

use creamui_bench::painter::raster_painter;
use creamui_bench::scenes;
use creamui_core::{Renderer, Size};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

const VIEWPORT: Size = Size {
    width: 800.0,
    height: 1000.0,
};

fn bench_scroll_triggered_repaint(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll/repaint_after_scroll_offset_change");
    for &count in &[1_000usize, 10_000] {
        let mut renderer = Renderer::new();
        let mut painter = raster_painter(800, 1000);
        renderer.render(scenes::chat(count), VIEWPORT, &mut painter);
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                renderer.repaint_focused(&mut painter, None, false);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_scroll_triggered_repaint);
criterion_main!(benches);
