use creamui_core::{Painter, Rect};
use creamui_image::ImageData;
use creamui_render::{Damage, Rasterizer, SceneRecorder};
use creamui_theme::{Color, ColorScheme};
use std::time::Instant;

fn memory_kib() -> (u64, u64) {
    let status = std::fs::read_to_string("/proc/self/status").expect("process status");
    let field = |name: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse().ok())
            .expect("memory field")
    };
    (field("VmRSS:"), field("RssAnon:"))
}

fn main() {
    let path = std::env::args().nth(1).expect("image path");
    let image = ImageData::from_path(path).expect("image decodes");
    let mut recorder = SceneRecorder::new();
    recorder.begin(
        640,
        360,
        1.0,
        Color::rgb(20, 20, 24),
        ColorScheme::default(),
    );
    recorder.draw_image(
        Rect {
            x: 16.0,
            y: 24.0,
            width: 240.0,
            height: 135.0,
        },
        image.image(),
        Some(Color::rgba(35, 140, 225, 180)),
    );
    let frame = recorder.finish();
    let mut raster = Rasterizer::new(640, 360);
    let (rss, anon) = memory_kib();
    println!("before rss_kib={rss} anon_kib={anon}");
    let start = Instant::now();
    raster.render(&frame, &Damage::Full);
    println!("first_ms={:.3}", start.elapsed().as_secs_f64() * 1_000.0);
    let (rss, anon) = memory_kib();
    println!("after rss_kib={rss} anon_kib={anon}");
    let start = Instant::now();
    for _ in 0..30 {
        raster.render(&frame, &Damage::Full);
    }
    println!(
        "warm_ms={:.3}",
        start.elapsed().as_secs_f64() * 1_000.0 / 30.0
    );
    let checksum: u64 = raster.pixmap().data().iter().map(|byte| *byte as u64).sum();
    println!("pixel_sum={checksum}");
}
