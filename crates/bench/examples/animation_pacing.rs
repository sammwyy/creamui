//! Runs an animated window for a few seconds and reports how evenly frames
//! were produced: rate, mean interval, jitter and the share of intervals
//! more than 25% off the display's refresh interval.
//!
//! ```sh
//! cargo run -p creamui-bench --release --example animation_pacing -- [SECONDS] [--cpu]
//! ```

use creamui_core::{BoxedWidget, Painter, Rect, Size, Style, Widget};
use creamui_render::{AppBuilder, RenderBackend, WindowOptions};
use creamui_theme::Color;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

struct Pulse {
    frames: Rc<RefCell<Vec<Instant>>>,
}

impl Widget for Pulse {
    fn style(&self) -> Style {
        Style::new().width(200.0).height(200.0)
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let t = painter.animation_time();
        self.frames.borrow_mut().push(Instant::now());
        let alpha = ((t * 1000.0) as u32 % 256) as u8;
        painter.fill_rect(rect, Color::rgba(200, 80, 80, alpha), 8.0);
    }
}

fn main() {
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let backend = if std::env::args().any(|arg| arg == "--cpu") {
        RenderBackend::Cpu
    } else {
        RenderBackend::Gpu
    };
    let refresh_hz: f64 = std::env::var("REFRESH_HZ")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60.0);
    let frames = Rc::new(RefCell::new(Vec::<Instant>::new()));
    let build = {
        let frames = frames.clone();
        move |_: Size| -> BoxedWidget {
            Box::new(Pulse {
                frames: frames.clone(),
            })
        }
    };
    let report = frames.clone();
    AppBuilder::new()
        .window(
            WindowOptions {
                title: "animation pacing".into(),
                width: 200,
                height: 200,
                backend,
                ..Default::default()
            },
            Color::rgb(20, 20, 24),
            move |handle| {
                let app = handle.app();
                let exit = app.clone();
                app.spawn_background(
                    move || std::thread::sleep(Duration::from_secs(seconds)),
                    move |()| exit.exit(),
                );
            },
            build,
        )
        .run();

    let frames = report.borrow();
    let skip = frames.len().min(10);
    let intervals: Vec<f64> = frames[skip..]
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).as_secs_f64() * 1000.0)
        .collect();
    let count = intervals.len() as f64;
    let mean = intervals.iter().sum::<f64>() / count;
    let jitter = (intervals.iter().map(|i| (i - mean).powi(2)).sum::<f64>() / count).sqrt();
    let target = 1000.0 / refresh_hz;
    let off = intervals
        .iter()
        .filter(|i| (**i - target).abs() > target * 0.25)
        .count() as f64;
    println!(
        "fps={:.1} mean_ms={mean:.3} jitter_ms={jitter:.3} off_by_25%={:.1}% (refresh {refresh_hz} Hz)",
        1000.0 / mean,
        off / count * 100.0
    );
}
