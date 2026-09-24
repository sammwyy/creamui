//! Bytes uploaded to the GPU and render time per frame for a scrolled
//! 1,000-message list on an offscreen target, when nothing changes, when
//! one small element changes (as a blinking caret does) and when the list
//! scrolls by one wheel step.
//!
//! ```sh
//! cargo run -p creamui-bench --release --example gpu_uploads
//! ```

use creamui_bench::painter::FramePipeline;
use creamui_bench::scenes;
use creamui_core::{metrics, Point, Renderer, Size};
use creamui_render::{DisplayList, HeadlessGpu};
use creamui_widgets::ScrollController;
use std::time::Instant;

const RUNS: u32 = 50;

fn measure(gpu: &mut HeadlessGpu, lists: &[DisplayList]) -> (u64, f64) {
    gpu.render(&lists[0]);
    metrics::reset_frame_metrics();
    let started = Instant::now();
    for run in 0..RUNS as usize {
        gpu.render(&lists[(run + 1) % lists.len()]);
    }
    let elapsed = started.elapsed().as_secs_f64() * 1000.0 / RUNS as f64;
    (
        metrics::frame_metrics().gpu_upload_bytes / RUNS as u64,
        elapsed,
    )
}

fn main() {
    let mut gpu = HeadlessGpu::new().expect("a GPU adapter");
    println!("GPU adapter: {}", gpu.adapter_name());
    let viewport = Size {
        width: 800.0,
        height: 1000.0,
    };
    let controller = ScrollController::new(400.0);
    let mut renderer = Renderer::new();
    renderer.update(
        scenes::scrolled_chat(1_000, 800.0, 1000.0, controller.clone()),
        viewport,
    );
    let mut frames = FramePipeline::new(800, 1000, 1.0);

    let mut record = |pointer: Option<Point>, offset: f32| {
        frames.set_pointer(pointer);
        controller.set(offset);
        frames.record(&renderer)
    };
    let thumb = Some(Point { x: 795.0, y: 500.0 });
    for (name, lists) in [
        ("unchanged", vec![record(None, 400.0), record(None, 400.0)]),
        (
            "small change",
            vec![record(None, 400.0), record(thumb, 400.0)],
        ),
        (
            "scroll 40px",
            vec![record(None, 400.0), record(None, 440.0)],
        ),
    ] {
        let (bytes, ms) = measure(&mut gpu, &lists);
        println!("{name:<14} {bytes:>9} bytes/frame {ms:>8.3} ms/frame");
    }
}
