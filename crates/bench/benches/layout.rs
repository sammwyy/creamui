//! `taffy` layout cost for a realistic nested grid, both on initial mount
//! and on a viewport resize.

use creamui_bench::painter::NoopPainter;
use creamui_bench::scenes;
use creamui_core::{Renderer, Size};
use criterion::{criterion_group, criterion_main, Criterion};

const VIEWPORT: Size = Size {
    width: 1920.0,
    height: 1080.0,
};

fn bench_grid_initial_layout(c: &mut Criterion) {
    c.bench_function("layout/grid_100x100_initial_mount", |b| {
        b.iter(|| {
            let mut renderer = Renderer::new();
            let mut painter = NoopPainter;
            renderer.render(scenes::grid(100, 100), VIEWPORT, &mut painter);
        });
    });
}

fn bench_grid_viewport_resize(c: &mut Criterion) {
    c.bench_function("layout/grid_100x100_viewport_resize", |b| {
        let mut renderer = Renderer::new();
        let mut painter = NoopPainter;
        renderer.render(scenes::grid(100, 100), VIEWPORT, &mut painter);
        let mut toggle = false;
        b.iter(|| {
            toggle = !toggle;
            let viewport = if toggle {
                Size {
                    width: 1600.0,
                    height: 900.0,
                }
            } else {
                VIEWPORT
            };
            renderer.render(scenes::grid(100, 100), viewport, &mut painter);
        });
    });
}

criterion_group!(
    benches,
    bench_grid_initial_layout,
    bench_grid_viewport_resize
);
criterion_main!(benches);
