//! Prints what each pipeline stage costs for the benchmark scenes, on the
//! CPU rasterizer and on an offscreen GPU target, plus process memory.
//!
//! ```sh
//! cargo run -p creamui-bench --release --example frame_report
//! CUI_GPU_FALLBACK=1 cargo run -p creamui-bench --release --example frame_report
//! ```

use creamui_bench::metrics::measure;
use creamui_bench::painter::FramePipeline;
use creamui_bench::scenes;
use creamui_core::{BoxedWidget, Point, Renderer, Size};
use creamui_render::HeadlessGpu;
use std::time::{Duration, Instant};

const RUNS: u32 = 20;

type SceneBuilder = fn() -> BoxedWidget;

fn average(mut f: impl FnMut()) -> Duration {
    let started = Instant::now();
    for _ in 0..RUNS {
        f();
    }
    started.elapsed() / RUNS
}

fn ms(d: Duration) -> String {
    format!("{:>8.3}", d.as_secs_f64() * 1000.0)
}

fn rss_mb() -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                let kb: f32 = line
                    .strip_prefix("VmRSS:")?
                    .trim()
                    .trim_end_matches("kB")
                    .trim()
                    .parse()
                    .ok()?;
                Some(format!("{:.1} MB", kb / 1024.0))
            })
        })
        .unwrap_or_else(|| "n/a".into())
}

fn main() {
    let mut gpu = HeadlessGpu::new()
        .map_err(|err| eprintln!("GPU column disabled: {err}"))
        .ok();
    if let Some(gpu) = &gpu {
        println!("GPU adapter: {}", gpu.adapter_name());
    }
    println!(
        "all times in ms, averaged over {RUNS} runs; RSS {}\n",
        rss_mb()
    );
    println!(
        "{:<16} {:>7} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>9}",
        "scene", "items", "layout", "record", "cpu_full", "cpu_idle", "cpu_hovr", "gpu", "hover_px"
    );

    let scenes: [(&str, SceneBuilder); 5] = [
        ("dashboard", scenes::dashboard),
        ("grid_100x100", || scenes::grid(100, 100)),
        ("wide_10k", || scenes::wide_tree(10_000)),
        ("chat_1k", || scenes::chat(1_000)),
        ("hover_list_1k", || scenes::hover_list(1_000)),
    ];
    let viewport = Size {
        width: 1440.0,
        height: 900.0,
    };
    for (name, build) in scenes {
        let mut renderer = Renderer::new();
        let layout = average(|| renderer.update(build(), viewport));
        let mut frames = FramePipeline::new(1440, 900, 1.0);
        let mut items = 0;
        let record = average(|| items = frames.record(&renderer).items.len());
        let cpu_full = average(|| {
            let mut fresh = FramePipeline::new(1440, 900, 1.0);
            fresh.frame(&renderer);
        });
        frames.frame(&renderer);
        let cpu_idle = average(|| {
            frames.frame(&renderer);
        });
        let mut row = 0;
        let mut hover_px = 0.0;
        let cpu_hover = average(|| {
            row = (row + 1) % 30;
            frames.set_pointer(Some(Point {
                x: 12.0,
                y: row as f32 * 24.0 + 6.0,
            }));
            hover_px = frames.frame(&renderer).damaged_pixels;
        });
        let gpu_time = gpu.as_mut().map_or("     n/a".into(), |gpu| {
            let list = frames.record(&renderer);
            gpu.render(&list);
            ms(average(|| {
                gpu.render(&list);
            }))
        });
        println!(
            "{name:<16} {items:>7} {} {} {} {} {} {} {hover_px:>9.0}",
            ms(layout),
            ms(record),
            ms(cpu_full),
            ms(cpu_idle),
            ms(cpu_hover),
            gpu_time
        );
    }

    let mut renderer = Renderer::new();
    renderer.update(scenes::dashboard(), viewport);
    let mut frames = FramePipeline::new(1440, 900, 1.0);
    let (_, metrics) = measure(|| frames.frame(&renderer));
    println!("\ndashboard first frame counters: {metrics:#?}");
    println!(
        "text cache: {} layouts, {} glyphs; RSS {}",
        frames.recorder.text().cached_layouts(),
        frames.recorder.text().cached_glyphs(),
        rss_mb()
    );
}
