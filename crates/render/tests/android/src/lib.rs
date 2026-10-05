use creamui_core::{BoxedWidget, Painter, Rect, Size, Style, Widget};
use creamui_reactive::Signal;
use creamui_render::{AppBuilder, PopupOptions, RenderBackend, WindowOptions};
use creamui_theme::Color;
use std::rc::Rc;

struct ColorButton {
    viewport: Size,
    count: Signal<u32>,
    color: Color,
}

impl Widget for ColorButton {
    fn style(&self) -> Style {
        Style::new()
            .width(self.viewport.width)
            .height(self.viewport.height)
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let half = rect.height * 0.5;
        painter.fill_rect(
            Rect {
                height: half,
                ..rect
            },
            Color::rgb(200, 40, 60),
            0.0,
        );
        painter.fill_rect(
            Rect {
                y: rect.y + half,
                height: half,
                ..rect
            },
            self.color,
            0.0,
        );
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        let count = self.count.clone();
        Some(Rc::new(move || {
            count.set(count.peek() + 1);
            log::info!("lifecycle-check: count={}", count.peek());
        }))
    }
}

fn button(viewport: Size, count: Signal<u32>) -> BoxedWidget {
    let color = match count.get() % 3 {
        0 => Color::rgb(40, 60, 200),
        1 => Color::rgb(40, 200, 60),
        _ => Color::rgb(200, 160, 40),
    };
    Box::new(ColorButton {
        viewport,
        count,
        color,
    })
}

#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("creamui-lifecycle")
            .with_max_level(log::LevelFilter::Debug)
            .with_filter(
                android_logger::FilterBuilder::new()
                    .parse("info,creamui_render=debug")
                    .build(),
            ),
    );
    std::panic::set_hook(Box::new(|panic| {
        log::error!("lifecycle-check: panic: {panic}")
    }));
    let count = Signal::new(0);
    let backend = if cfg!(feature = "cpu") {
        RenderBackend::Cpu
    } else {
        RenderBackend::Gpu
    };
    AppBuilder::new()
        .window(
            WindowOptions {
                backend,
                ..WindowOptions::default()
            },
            Color::rgb(0, 0, 0),
            |window| {
                log::info!("lifecycle-check: ready");
                window.app().append_window(
                    WindowOptions::default(),
                    Color::rgb(0, 0, 0),
                    |_| {
                        panic!("additional Android window was accepted");
                    },
                    |viewport| button(viewport, Signal::new(0)),
                );
                window.app().append_popup(
                    WindowOptions::default(),
                    PopupOptions::new(
                        window.clone(),
                        Rect {
                            x: 0.0,
                            y: 0.0,
                            width: 40.0,
                            height: 40.0,
                        },
                    ),
                    Color::rgb(0, 0, 0),
                    |_| {
                        panic!("Android native popup was accepted");
                    },
                    |viewport| button(viewport, Signal::new(0)),
                );
            },
            move |viewport| button(viewport, count.clone()),
        )
        .run_android(app);
}
