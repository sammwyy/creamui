//! Cost of the text shaping cache (`creamui_render::ShapeCache`) —
//! REFACTOR.md 15.2/15.5's shaping cache: re-shaping already-cached text
//! should cost a hashmap lookup, not a `fontdue` layout pass, and that
//! lookup should stay flat regardless of how much other distinct text is
//! cached alongside it.

use creamui_render::ShapeCache;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_shape_cache_miss(c: &mut Criterion) {
    c.bench_function("text_shape/cache_miss", |b| {
        let mut cache = ShapeCache::new();
        let mut tick = 0u32;
        b.iter(|| {
            tick = tick.wrapping_add(1);
            cache.shape(&format!("hello world {tick}"), 16.0, 400.0, None, false);
        });
    });
}

/// Re-shaping the same already-cached text must cost the same whether 10
/// or 10,000 other distinct texts are already warm in the cache.
fn bench_shape_cache_hit_independent_of_cache_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_shape/cache_hit");
    for &count in &[10usize, 1_000, 10_000] {
        let mut cache = ShapeCache::with_capacity(count + 1);
        for i in 0..count {
            cache.shape(&format!("distinct text {i}"), 16.0, 400.0, None, false);
        }
        cache.shape("target text", 16.0, 400.0, None, false);

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                cache.shape("target text", 16.0, 400.0, None, false);
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_shape_cache_miss,
    bench_shape_cache_hit_independent_of_cache_size
);
criterion_main!(benches);
