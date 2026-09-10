//! A background CreamUI application controlled from the native system tray.
//!
//! Closing the window destroys it but leaves the application alive. Use the
//! tray menu to create it again, increment its reactive counter while no
//! window exists, or exit the process.

use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_render::{AppBuilder, AppHandle, TrayBuilder, TrayIcon, WindowHandle, WindowOptions};
use creamui_theme::{use_theme, Theme};
use creamui_widgets::layout::{Align, Flex, Justify};
use creamui_widgets::{Button, Text};
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    let count = Signal::new(0_i32);
    let window = Rc::new(RefCell::new(None::<WindowHandle>));

    let show_window = window.clone();
    let increment_from_tray = count.clone();
    let open_from_start = {
        let count = count.clone();
        let window = window.clone();
        move |app: AppHandle| open_window(&app, count.clone(), window.clone())
    };
    let open_from_tray = {
        let count = count.clone();
        let window = window.clone();
        move |app: &AppHandle| open_window(app, count.clone(), window.clone())
    };

    AppBuilder::new()
        // Without this, the legacy AppBuilder behavior is to exit after its
        // last real window is closed. A tray app owns its lifetime instead.
        .keep_running()
        .on_started(open_from_start)
        .tray(
            TrayBuilder::new(tray_icon())
                .tooltip("CreamUI tray example")
                .item("show", "Show window", move |app| {
                    let live_window = show_window.borrow().clone();
                    if let Some(window) = live_window.filter(WindowHandle::is_open) {
                        window.show();
                    } else {
                        open_from_tray(app);
                    }
                })
                .item("increment", "Increment (+1)", move |_| {
                    increment_from_tray.update(|value| *value += 1);
                })
                .quit_item("quit", "Quit"),
        )
        .run();
}

fn open_window(app: &AppHandle, count: Signal<i32>, window: Rc<RefCell<Option<WindowHandle>>>) {
    let ready_window = window.clone();
    let count_for_ui = count.clone();
    let window_for_ui = window.clone();
    app.append_window(
        WindowOptions {
            title: "CreamUI tray example".into(),
            width: 480,
            height: 300,
            theme: Theme::dark(),
            ..Default::default()
        },
        Theme::dark().surface,
        move |handle| *ready_window.borrow_mut() = Some(handle),
        move |size: Size| -> BoxedWidget {
            let theme = use_theme();
            let count_for_button = count_for_ui.clone();
            let window_for_button = window_for_ui.clone();
            Box::new(
                Flex::column()
                    .size(size.width, size.height)
                    .gap(14.0)
                    .justify(Justify::Center)
                    .align(Align::Center)
                    .background(theme.surface)
                    .child(Box::new(Text::new("CreamUI system tray").font_size(28.0)))
                    .child(Box::new(Text::new(format!("Count: {}", count.get()))))
                    .child(Box::new(Button::new("Increment", move || {
                        count_for_button.update(|value| *value += 1);
                    })))
                    .child(Box::new(Button::new("Close window", move || {
                        if let Some(window) = window_for_button.borrow().as_ref() {
                            window.close();
                        }
                    }))),
            )
        },
    );
}

/// A tiny dependency-free 32×32 RGBA icon: CreamUI purple with a white C.
fn tray_icon() -> TrayIcon {
    const SIZE: u32 = 32;
    let mut rgba = vec![0_u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - 15.5;
            let dy = y as f32 - 15.5;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance <= 15.0 {
                let index = ((y * SIZE + x) * 4) as usize;
                rgba[index..index + 4].copy_from_slice(&[113, 73, 224, 255]);
                // A simple, high-contrast C that stays legible at panel size.
                if (8.0..=12.0).contains(&distance) && (x < 13 || y < 10 || y > 21) {
                    rgba[index..index + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
    }
    TrayIcon::from_rgba(rgba, SIZE, SIZE).expect("the generated tray icon is valid RGBA")
}
