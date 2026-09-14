//! Cost of changing one leaf's text content in a large list.

use creamui_bench::painter::raster_painter;
use creamui_bench::scenes;
use creamui_core::{Renderer, Size};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

const VIEWPORT: Size = Size {
    width: 800.0,
    height: 2_000_000.0,
};

fn bench_single_text_change(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_update/single_leaf_text_change");
    for &count in &[1_000usize, 10_000] {
        let mut renderer = Renderer::new();
        let mut painter = raster_painter(800, 600);
        renderer.render(
            scenes::chat_with_message(count, 0, "message 0"),
            VIEWPORT,
            &mut painter,
        );
        let mut tick = 0u64;
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                tick += 1;
                renderer.render(
                    scenes::chat_with_message(count, 0, &format!("updated {tick}")),
                    VIEWPORT,
                    &mut painter,
                );
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_single_text_change);
criterion_main!(benches);
