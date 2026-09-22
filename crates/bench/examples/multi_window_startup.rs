//! Times how long it takes to open N GPU-backend windows in one process —
//! the path `GpuContext` (`crates/render/src/gpu.rs`) speeds up, since only
//! the first window pays for `request_adapter`/`request_device`.
//!
//! ```sh
//! cargo run -p creamui-bench --release --example multi_window_startup -- 6
//! CUI_DEBUG=1 cargo run -p creamui-bench --release --example multi_window_startup -- 6
//! ```

use creamui_core::{BoxedWidget, Size, Style};
use creamui_render::{AppBuilder, AppHandle, WindowOptions};
use creamui_theme::Color;
use creamui_widgets::raw::RawView;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

fn build_ui(size: Size) -> BoxedWidget {
    Box::new(RawView::new(
        Style::new().width(size.width).height(size.height),
    ))
}

fn main() {
    let count: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);

    let start = Instant::now();
    let times = Rc::new(RefCell::new(Vec::<Duration>::with_capacity(count)));
    let app_handle: Rc<RefCell<Option<AppHandle>>> = Rc::new(RefCell::new(None));

    let mut builder = AppBuilder::new().on_started({
        let app_handle = app_handle.clone();
        move |app| *app_handle.borrow_mut() = Some(app)
    });
    for i in 0..count {
        let times = times.clone();
        let app_handle = app_handle.clone();
        builder = builder.window(
            WindowOptions {
                title: format!("bench window {i}"),
                width: 200,
                height: 150,
                ..Default::default()
            },
            Color::rgb(20, 20, 24),
            move |_handle| {
                times.borrow_mut().push(start.elapsed());
                if times.borrow().len() == count {
                    if let Some(app) = app_handle.borrow().as_ref() {
                        app.exit();
                    }
                }
            },
            build_ui,
        );
    }
    builder.run();

    let times = times.borrow();
    println!("windows: {count}");
    for (i, t) in times.iter().enumerate() {
        println!("  window {i:>2} ready at {:>8.2} ms", t.as_secs_f64() * 1000.0);
    }
    let total = times.last().copied().unwrap_or_default();
    let per_window = total.as_secs_f64() * 1000.0 / count as f64;
    println!(
        "total: {:.2} ms, avg/window: {:.2} ms",
        total.as_secs_f64() * 1000.0,
        per_window
    );
}
