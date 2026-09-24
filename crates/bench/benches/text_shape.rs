//! Cost of recording text: a repeated label must cost a cache lookup, not a
//! layout pass.

use creamui_core::{Painter, Rect, TextAlign};
use creamui_render::SceneRecorder;
use creamui_theme::{Color, ColorScheme};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

const RECT: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 400.0,
    height: 20.0,
};

fn begin(recorder: &mut SceneRecorder) {
    recorder.begin(400, 20, 1.0, Color::rgb(0, 0, 0), ColorScheme::default());
}

fn bench_layout_miss(c: &mut Criterion) {
    c.bench_function("text_shape/cache_miss", |b| {
        let mut recorder = SceneRecorder::new();
        begin(&mut recorder);
        let mut tick = 0u32;
        b.iter(|| {
            tick = tick.wrapping_add(1);
            let text = format!("hello world {tick}");
            recorder.fill_text(
                RECT,
                &text,
                Color::rgb(255, 255, 255),
                16.0,
                TextAlign::Start,
            );
        });
    });
}

fn bench_layout_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_shape/cache_hit");
    for &count in &[10usize, 1_000, 10_000] {
        let mut recorder = SceneRecorder::new();
        begin(&mut recorder);
        for i in 0..count {
            let text = format!("distinct text {i}");
            recorder.fill_text(
                RECT,
                &text,
                Color::rgb(255, 255, 255),
                16.0,
                TextAlign::Start,
            );
        }
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                recorder.fill_text(
                    RECT,
                    "distinct text 3",
                    Color::rgb(255, 255, 255),
                    16.0,
                    TextAlign::Start,
                );
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_layout_miss, bench_layout_hit);
criterion_main!(benches);
