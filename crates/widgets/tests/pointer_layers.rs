use creamui_core::{
    BoxedWidget, CursorIcon, KeyInput, Painter, Point, Rect, Renderer, Size, Style, Styled,
    TextAlign, Widget,
};
use creamui_theme::{Color, Theme, ThemeProvider};
use creamui_widgets::{RawScrollView, RawView, ScrollController, Select, SelectController};
use std::rc::Rc;

struct BackgroundInteractions;

impl Widget for BackgroundInteractions {
    fn style(&self) -> Style {
        Style::new().width(320.0).height(260.0)
    }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
    fn focusable(&self) -> bool {
        true
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        Some(Rc::new(|_| {}))
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        Some(Rc::new(|| {}))
    }
    fn on_click_at(&self) -> Option<Rc<dyn Fn(Point)>> {
        Some(Rc::new(|_| {}))
    }
    fn on_drag(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        Some(Rc::new(|_, _| {}))
    }
    fn on_drag_start(&self) -> Option<Rc<dyn Fn(Point, Rect)>> {
        Some(Rc::new(|_, _| {}))
    }
    fn on_hover(&self) -> Option<Rc<dyn Fn(bool)>> {
        Some(Rc::new(|_| {}))
    }
    fn on_scroll(&self) -> Option<Rc<dyn Fn(f32)>> {
        Some(Rc::new(|_| {}))
    }
    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Pointer)
    }
    fn on_window_drag(&self) -> Option<Rc<dyn Fn()>> {
        Some(Rc::new(|| {}))
    }
}

#[derive(Default)]
struct TextPainter(Vec<String>);

impl Painter for TextPainter {
    fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
    fn fill_text(&mut self, _: Rect, text: &str, _: Color, _: f32, _: TextAlign) {
        self.0.push(text.to_owned());
    }
}

fn build(controller: &SelectController) -> BoxedWidget {
    Box::new(
        RawView::new(creamui_widgets::layout::column(0.0))
            .child(Box::new(
                Select::controlled(
                    &[
                        "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine",
                    ],
                    controller.clone(),
                )
                .searchable()
                .width(176.0)
                .height(34.0),
            ))
            .child(Box::new(BackgroundInteractions)),
    )
}

#[test]
fn select_surfaces_block_every_background_pointer_channel() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        let controller = SelectController::default();
        controller.set_open(true);
        let mut renderer = Renderer::new();
        let mut painter = TextPainter::default();
        let viewport = Size {
            width: 320.0,
            height: 340.0,
        };
        let scene = renderer.render(build(&controller), viewport, &mut painter);
        for point in [Point { x: 1.0, y: 35.0 }, Point { x: 280.0, y: 250.0 }] {
            assert!(scene.hit_test(point).is_some());
            assert!(scene.hit_test_at(point).is_none());
            assert!(scene.focus_hit_test(point).is_none());
            assert!(scene.drag_hit_test(point).is_none());
            assert!(scene.drag_start_at(point).is_none());
            assert!(scene.scroll_hit_test(point).is_none());
            assert!(scene.hover_hit_test(point).is_none());
            assert!(scene.window_drag_at(point).is_none());
            assert_ne!(scene.cursor_hit_test(point), Some(CursorIcon::Pointer));
        }
        scene.hit_test(Point { x: 1.0, y: 35.0 }).unwrap()();
        assert!(controller.is_open());
        let search = Point { x: 20.0, y: 50.0 };
        let focus = scene.focus_hit_test(search).unwrap();
        assert!(scene.focus_accepts_text_input(focus));
        assert!(scene.drag_hit_test(search).is_some());
        let list = scene.scroll_hit_test(Point { x: 20.0, y: 100.0 }).unwrap();
        scene.on_scroll_at(list).unwrap()(40.0);
        assert_eq!(controller.scroll().offset(), 40.0);
        scene.hit_test(Point { x: 280.0, y: 250.0 }).unwrap()();
        assert!(!controller.is_open());
        let scene = renderer.render(build(&controller), viewport, &mut painter);
        assert!(scene.hit_test_at(search).is_some());
        assert!(scene.drag_hit_test(search).is_some());
        assert!(scene.scroll_hit_test(search).is_some());
        assert!(scene.hover_hit_test(search).is_some());
        assert_eq!(scene.cursor_hit_test(search), Some(CursorIcon::Pointer));
    });
}

#[test]
fn unmatched_search_keeps_a_visible_shielded_empty_state() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        let controller = SelectController::default();
        controller.set_open(true);
        controller.query().set_value("unmatched");
        let mut renderer = Renderer::new();
        let mut painter = TextPainter::default();
        let scene = renderer.render(
            build(&controller),
            Size {
                width: 320.0,
                height: 340.0,
            },
            &mut painter,
        );
        assert!(painter.0.iter().any(|text| text == "No results"));
        let empty = Point { x: 20.0, y: 95.0 };
        scene.hit_test(empty).unwrap()();
        assert!(controller.is_open());
        assert_eq!(controller.selected(), 0);
        assert!(scene.focus_hit_test(empty).is_none());
        assert!(scene.drag_hit_test(empty).is_none());
        assert!(scene.scroll_hit_test(empty).is_none());
    });
}

#[test]
fn page_scrollbars_stay_below_select_portals() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        let controller = SelectController::default();
        controller.set_open(true);
        let root = RawScrollView::controlled(
            Style::new().width(176.0).height(120.0),
            ScrollController::new(0.0),
        )
        .child(Box::new(
            Select::controlled(&["One", "Two"], controller)
                .searchable()
                .width(176.0)
                .height(34.0),
        ))
        .child(Box::new(BackgroundInteractions));
        let scene = Renderer::new().render(
            Box::new(root),
            Size {
                width: 320.0,
                height: 340.0,
            },
            &mut TextPainter::default(),
        );
        let point = Point { x: 170.0, y: 50.0 };
        let drag = scene.drag_hit_test(point).unwrap();
        let (rect, _) = scene.draggable_at(drag).unwrap();
        assert!(rect.width > 100.0);
        let focus = scene.focus_hit_test(point).unwrap();
        assert!(scene.focus_accepts_text_input(focus));
        assert!(scene.scroll_hit_test(point).is_none());
    });
}

#[test]
fn retained_page_scrollbars_paint_before_select_portals() {
    use creamui_core::layout::{Dimension, Position};
    use creamui_core::runtime::{mount_legacy_widget, Runtime};
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        let controller = SelectController::default();
        controller.set_open(true);
        let root = RawScrollView::controlled(
            Style::new().width(176.0).height(120.0),
            ScrollController::new(0.0),
        )
        .child(Box::new(
            Select::controlled(&["One", "Two"], controller)
                .searchable()
                .width(176.0)
                .height(34.0),
        ))
        .child(Box::new(BackgroundInteractions));
        let mut runtime = Runtime::new();
        let root = mount_legacy_widget(&mut runtime.transaction(), Box::new(root), None);
        runtime.set_root(Some(root));
        let scrollbar = runtime.get(root).unwrap().children.as_slice()[1];
        runtime.compute_layout(Size {
            width: 320.0,
            height: 340.0,
        });
        runtime.rebuild_composite();
        runtime.rebuild_paint(&creamui_theme::ColorScheme::light());
        let order = runtime.paint_order();
        let scrollbar_rank = order.iter().position(|&node| node == scrollbar).unwrap();
        let popup_rank = order
            .iter()
            .position(|&id| {
                let node = runtime.get(id).unwrap();
                node.layout_style.position == Position::Absolute
                    && node.layout_style.size.width == Dimension::Percent(1.0)
            })
            .unwrap();
        assert!(scrollbar_rank < popup_rank);
    });
}
