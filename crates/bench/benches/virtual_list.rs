//! Cost of `creamui_core::HeightIndex`'s Fenwick-tree point updates and
//! offset queries as item count grows — REFACTOR.md 17.2's "efficient
//! offset correction" claim: `O(log n)`, not `O(n)`.

use creamui_core::{visible_range, HeightIndex};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_set_height(c: &mut Criterion) {
    let mut group = c.benchmark_group("virtual_list/set_height");
    for &count in &[1_000usize, 10_000, 100_000] {
        let mut index = HeightIndex::uniform(count, 28.0);
        let target = count / 2;
        let mut tick = 0.0f32;
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                tick += 1.0;
                index.set_height(target, 28.0 + tick);
            });
        });
    }
    group.finish();
}

fn bench_index_at_offset(c: &mut Criterion) {
    let mut group = c.benchmark_group("virtual_list/index_at_offset");
    for &count in &[1_000usize, 10_000, 100_000] {
        let index = HeightIndex::uniform(count, 28.0);
        let target_offset = index.total_height() * 0.5;
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| index.index_at_offset(target_offset));
        });
    }
    group.finish();
}

fn bench_visible_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("virtual_list/visible_range");
    for &count in &[1_000usize, 10_000, 100_000] {
        let index = HeightIndex::uniform(count, 28.0);
        let target_offset = index.total_height() * 0.5;
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| visible_range(&index, target_offset, 800.0, 4));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_set_height,
    bench_index_at_offset,
    bench_visible_range
);
criterion_main!(benches);
