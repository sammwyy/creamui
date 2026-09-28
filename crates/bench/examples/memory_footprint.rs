//! Opens N windows showing text and, optionally, a large image scaled down
//! to a thumbnail, waits for them to settle and prints the process's
//! resident memory and GPU memory (from DRM `fdinfo`, Linux only).
//!
//! ```sh
//! cargo run -p creamui-bench --release --example memory_footprint -- \
//!     [--windows N] [--font FAMILY] [--image PATH] [--cpu]
//! ```

use creamui_core::layout::{FlexDirection, Style};
use creamui_core::{BoxedWidget, Size, Styled};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_render::{AppBuilder, AppHandle, RenderBackend, WindowOptions};
use creamui_theme::Color;
use creamui_widgets::layout::fixed;
use creamui_widgets::raw::{RawText, RawView};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

const TEXT: &str =
    "The quick brown fox jumps over the lazy dog 0123456789 你好，世界 こんにちは 안녕하세요";

struct Args {
    windows: usize,
    font: Option<String>,
    image: Option<String>,
    cpu: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        windows: 1,
        font: None,
        image: None,
        cpu: false,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--windows" => args.windows = iter.next().and_then(|n| n.parse().ok()).unwrap_or(1),
            "--font" => args.font = iter.next(),
            "--image" => args.image = iter.next(),
            "--cpu" => args.cpu = true,
            other => panic!("unknown argument {other}"),
        }
    }
    args
}

fn status_kb(field: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find_map(|line| line.strip_prefix(field))
        .and_then(|rest| rest.split_whitespace().next()?.parse().ok())
        .unwrap_or(0)
}

/// Sums `drm-memory-*` per DRM client, counting each client once even if
/// several file descriptors share it.
fn drm_memory_kb() -> (u64, u64) {
    let (mut vram, mut gtt) = (0, 0);
    let mut clients = HashSet::new();
    let Ok(entries) = std::fs::read_dir("/proc/self/fdinfo") else {
        return (0, 0);
    };
    for entry in entries.flatten() {
        let info = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let field = |name: &str| {
            info.lines()
                .find_map(|line| line.strip_prefix(name))
                .map(|rest| rest.trim().to_owned())
        };
        let Some(client) = field("drm-client-id:") else {
            continue;
        };
        if !clients.insert(client) {
            continue;
        }
        let kib = |value: Option<String>| {
            value
                .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
                .unwrap_or(0)
        };
        vram += kib(field("drm-memory-vram:"));
        gtt += kib(field("drm-memory-gtt:"));
    }
    (vram, gtt)
}

fn build_ui(size: Size, image: Option<ImageData>) -> BoxedWidget {
    let mut root = RawView::new(Style {
        size: fixed(size.width, size.height),
        flex_direction: FlexDirection::Column,
        ..Default::default()
    })
    .child(Box::new(RawText::new(
        TEXT,
        Color::rgb(230, 230, 230),
        18.0,
    )));
    if let Some(data) = image {
        root = root.child(Box::new(Image::new(data).fit(ImageFit::Cover).with_style(
            Style {
                size: fixed(240.0, 135.0),
                flex_shrink: 0.0,
                ..Default::default()
            },
        )));
    }
    Box::new(root)
}

fn main() {
    let args = parse_args();
    if let Some(font) = &args.font {
        assert!(
            creamui_fonts::use_system_font(font),
            "no system font matches {font}"
        );
    }
    let image = args
        .image
        .as_ref()
        .map(|path| ImageData::from_path(path).expect("image decodes"));

    let app_handle: Rc<RefCell<Option<AppHandle>>> = Rc::new(RefCell::new(None));
    let ready = Rc::new(RefCell::new(0usize));
    let mut builder = AppBuilder::new().on_started({
        let app_handle = app_handle.clone();
        move |app| *app_handle.borrow_mut() = Some(app)
    });
    for i in 0..args.windows {
        let image = image.clone();
        let app_handle = app_handle.clone();
        let ready = ready.clone();
        let windows = args.windows;
        builder = builder.window(
            WindowOptions {
                title: format!("memory window {i}"),
                width: 640,
                height: 360,
                backend: if args.cpu {
                    RenderBackend::Cpu
                } else {
                    RenderBackend::Gpu
                },
                ..Default::default()
            },
            Color::rgb(20, 20, 24),
            move |_handle| {
                *ready.borrow_mut() += 1;
                if *ready.borrow() < windows {
                    return;
                }
                let app = app_handle.borrow().clone().expect("app started");
                let exit = app.clone();
                app.spawn_background(
                    || std::thread::sleep(Duration::from_secs(2)),
                    move |()| {
                        let (vram, gtt) = drm_memory_kb();
                        println!(
                            "rss_kb={} rss_anon_kb={} vram_kb={vram} gtt_kb={gtt}",
                            status_kb("VmRSS:"),
                            status_kb("RssAnon:"),
                        );
                        exit.exit();
                    },
                );
            },
            move |size| build_ui(size, image.clone()),
        );
    }
    builder.run();
}
