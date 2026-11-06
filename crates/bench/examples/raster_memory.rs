use creamui_core::{Painter, Rect};
use creamui_render::{Damage, Rasterizer, SceneRecorder};
use creamui_theme::{Color, ColorScheme};

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

fn frame(width: u32, height: u32, clipped: bool) -> creamui_render::DisplayList {
    let mut recorder = SceneRecorder::new();
    recorder.begin(
        width,
        height,
        1.0,
        Color::rgb(20, 20, 24),
        ColorScheme::default(),
    );
    if clipped {
        let area = Rect {
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: height as f32,
        };
        recorder.push_clip_rounded(area, 16.0);
        recorder.fill_rect(area, Color::rgb(200, 40, 40), 16.0);
        recorder.pop_clip();
    }
    recorder.finish()
}

fn main() {
    let mut raster = Rasterizer::new(1, 1);
    let (rss, anon) = memory_kib();
    println!("initial rss_kib={rss} anon_kib={anon}");
    raster.render(&frame(2048, 2048, true), &Damage::Full);
    let (rss, anon) = memory_kib();
    println!("large rss_kib={rss} anon_kib={anon}");
    raster.render(&frame(320, 200, false), &Damage::None);
    let (rss, anon) = memory_kib();
    println!("small rss_kib={rss} anon_kib={anon}");
}
