//! Times the startup steps of a single GPU window: resolving the default
//! font (building the system font index on first use) and the time from
//! process start until the window is ready. Run with `CUI_DEBUG=1` to also
//! log how long the GPU surface took.
//!
//! ```sh
//! CUI_DEBUG=1 cargo run -p creamui-bench --release --example startup
//! ```

use creamui_core::{BoxedWidget, Size, Style};
use creamui_fonts::{FontWeight, DEFAULT_FAMILY};
use creamui_render::{AppBuilder, WindowOptions};
use creamui_theme::Color;
use creamui_widgets::raw::RawView;
use std::time::Instant;

fn main() {
    let start = Instant::now();
    creamui_fonts::resolve(DEFAULT_FAMILY, FontWeight::Regular);
    let font = start.elapsed();
    AppBuilder::new()
        .window(
            WindowOptions {
                title: "startup".into(),
                width: 200,
                height: 150,
                ..Default::default()
            },
            Color::rgb(20, 20, 24),
            move |handle| {
                println!(
                    "font_ms={:.3} window_ready_ms={:.3}",
                    font.as_secs_f64() * 1000.0,
                    start.elapsed().as_secs_f64() * 1000.0
                );
                handle.app().exit();
            },
            |size: Size| -> BoxedWidget {
                Box::new(RawView::new(
                    Style::new().width(size.width).height(size.height),
                ))
            },
        )
        .run();
}
