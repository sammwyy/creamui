//! GPU frame cost on an offscreen target: instance building, glyph atlas
//! and image uploads, encoding, and waiting for the device. Skipped when no
//! adapter is available; `CUI_GPU_FALLBACK=1` measures a software adapter.

use creamui_bench::painter::FramePipeline;
use creamui_bench::scenes;
use creamui_core::{Renderer, Size};
use creamui_render::HeadlessGpu;
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_gpu_frame(c: &mut Criterion) {
    let mut gpu = match HeadlessGpu::new() {
        Ok(gpu) => gpu,
        Err(err) => {
            eprintln!("skipping GPU benchmarks: {err}");
            return;
        }
    };
    eprintln!("GPU benchmarks on {}", gpu.adapter_name());
    let viewport = Size {
        width: 1440.0,
        height: 900.0,
    };
    let mut group = c.benchmark_group("gpu/frame");
    for (name, scene) in [
        ("dashboard", scenes::dashboard()),
        ("grid_100x100", scenes::grid(100, 100)),
    ] {
        let mut renderer = Renderer::new();
        let mut frames = FramePipeline::new(1440, 900, 1.0);
        renderer.update(scene, viewport);
        let list = frames.record(&renderer);
        gpu.render(&list);
        group.bench_function(name, |b| b.iter(|| gpu.render(&list)));
    }
    group.finish();
}

criterion_group!(benches, bench_gpu_frame);
criterion_main!(benches);
