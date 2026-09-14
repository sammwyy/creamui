//! Cost of the GPU quad scene's incremental update path
//! (`creamui_render::gpu_scene::QuadStore`), independent of any real GPU
//! device — proves updating one quad stays cheap regardless of how many
//! other quads are already in the store.

use creamui_core::Rect;
use creamui_render::QuadInstance;
use creamui_render::QuadStore;
use creamui_theme::Color;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn instance(seed: f32) -> QuadInstance {
    QuadInstance::fill(
        Rect {
            x: seed,
            y: seed,
            width: 10.0,
            height: 10.0,
        },
        Color::rgb(1, 2, 3),
        4.0,
        false,
    )
}

fn bench_insert_many(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_scene/insert_many");
    for &count in &[1_000usize, 10_000, 50_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter(|| {
                let mut store = QuadStore::new();
                for i in 0..count {
                    store.insert(instance(i as f32));
                }
            });
        });
    }
    group.finish();
}

/// Updating one quad in an already-populated store must cost the same
/// whether the store holds 1,000 or 50,000 quads — the dirty range it
/// produces stays a single slot either way.
fn bench_single_update_independent_of_store_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_scene/single_update");
    for &count in &[1_000usize, 10_000, 50_000] {
        let mut store = QuadStore::new();
        let mut ids = Vec::with_capacity(count);
        for i in 0..count {
            ids.push(store.insert(instance(i as f32)));
        }
        store.take_dirty_range();
        let target = ids[count / 2];

        let mut tick = 0u8;
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                tick = tick.wrapping_add(1);
                store.update(target, instance(tick as f32));
                store.take_dirty_range();
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_insert_many,
    bench_single_update_independent_of_store_size
);
criterion_main!(benches);
