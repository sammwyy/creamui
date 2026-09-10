//! End-to-end test: build a themed widget tree, run it through
//! `creamui_core::render_frame` with a recording `Painter`, and verify both
//! painting and click hit-testing work together.

use creamui_core::layout::{AlignItems, Dimension, JustifyContent, Style};
use creamui_core::{
    render_frame, CursorIcon, Key, KeyInput, Modifiers, Painter, Point, Rect, Renderer, Size,
    TextAlign, Widget,
};
use creamui_reactive::Signal;
use creamui_theme::{Color, Theme};
use creamui_widgets::raw::{
    RawButton, RawCheckbox, RawScrollView, RawSlider, RawSwitch, RawView, TextSelection,
};
use creamui_widgets::themed::{
    tab_styles, Button, Checkbox, ColorPicker, DateTimePicker, Link, ListBox, ListView, Overlay,
    Popover, Pre, ProgressBar, ProgressRing, ScrollView, Select, Slider, TabColors, TabSizing,
    Table, Tabs, Text, TextArea, TextInput, TreeNode, TreeView,
};
use creamui_widgets::TableColumn;
use creamui_widgets::{
    ColorPickerController, DateTimeController, ScrollController, SelectController, TreeController,
};

#[derive(Default)]
struct RecordingPainter {
    hovered: bool,
    pressed: bool,
    filled_rects: Vec<(Rect, Color)>,
    stroked_rects: Vec<(Rect, Color)>,
    texts: Vec<String>,
    text_rects: Vec<Rect>,
}

impl Painter for RecordingPainter {
    fn hovered(&self, _: Rect) -> bool {
        self.hovered
    }
    fn pressed(&self, _: Rect) -> bool {
        self.pressed
    }
    fn fill_rect(&mut self, rect: Rect, color: Color, _corner_radius: f32) {
        self.filled_rects.push((rect, color));
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, _width: f32, _corner_radius: f32) {
        self.stroked_rects.push((rect, color));
    }

    fn fill_text(
        &mut self,
        rect: Rect,
        text: &str,
        _color: Color,
        _font_size: f32,
        _align: TextAlign,
    ) {
        self.texts.push(text.to_string());
        self.text_rects.push(rect);
    }
}

#[derive(Default)]
struct FamilyPainter {
    families: Vec<String>,
}

impl Painter for FamilyPainter {
    fn hovered(&self, _: Rect) -> bool {
        false
    }

    fn pressed(&self, _: Rect) -> bool {
        false
    }

    fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}

    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}

    fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}

    fn fill_text_font(
        &mut self,
        _: Rect,
        _: &str,
        _: Color,
        _: f32,
        _: TextAlign,
        family: Option<&str>,
        _: bool,
        _: bool,
    ) {
        self.families.push(family.unwrap_or_default().to_owned());
    }

    fn fill_text_selected_font(
        &mut self,
        _: Rect,
        _: &str,
        _: Color,
        _: Color,
        _: std::ops::Range<usize>,
        _: f32,
        _: TextAlign,
        family: Option<&str>,
    ) {
        self.families.push(family.unwrap_or_default().to_owned());
    }
}

#[test]
fn themed_text_widgets_pass_overridden_font_families_to_painting() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(Theme::light()));
        let mut painter = FamilyPainter::default();
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 60.0,
        };

        TextInput::new("input", |_| {})
            .selection(
                TextSelection {
                    anchor: 0,
                    focus: 2,
                },
                |_| {},
            )
            .font_family("input-family")
            .paint(&mut painter, rect);
        TextArea::new("area", |_| {})
            .selection(
                TextSelection {
                    anchor: 0,
                    focus: 2,
                },
                |_| {},
            )
            .font_family("area-family")
            .paint(&mut painter, rect);
        Link::new("link", || {})
            .font_family("link-family")
            .paint(&mut painter, rect);
        Pre::new("pre")
            .font_family("pre-family")
            .paint(&mut painter, rect);

        for family in ["input-family", "area-family", "link-family", "pre-family"] {
            assert!(painter.families.iter().any(|seen| seen == family));
        }
    });
}

#[test]
fn button_interactions_use_theme_tokens_and_disabled_controls_skip_focus() {
    let theme = Theme::light();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let clicks = Signal::new(0);
        let build = || {
            let count = clicks.clone();
            Box::new(
                RawView::new(creamui_widgets::layout::column(10.))
                    .child(Box::new(Button::new("Enabled", move || {
                        count.update(|v| *v += 1)
                    })))
                    .child(Box::new(
                        Button::new("Disabled", || panic!("disabled activated")).disabled(true),
                    )),
            ) as creamui_core::BoxedWidget
        };
        let mut painter = RecordingPainter {
            hovered: true,
            ..Default::default()
        };
        let size = Size {
            width: 320.,
            height: 180.,
        };
        let scene = render_frame(build(), size, &mut painter);
        assert!(painter
            .filled_rects
            .iter()
            .any(|(_, color)| *color == theme.accent_hover));
        assert_eq!(scene.next_focus(None, false), Some(0));
        assert_eq!(scene.next_focus(Some(0), false), Some(0));
        assert_eq!(scene.next_focus(None, true), Some(0));
        scene.on_key_at(0).unwrap()(KeyInput {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        assert_eq!(clicks.get(), 1);
        painter.filled_rects.clear();
        painter.pressed = true;
        render_frame(build(), size, &mut painter);
        assert!(painter
            .filled_rects
            .iter()
            .any(|(_, color)| *color == theme.accent_pressed));
    });
}

#[test]
fn switch_checkbox_and_slider_support_keyboard_navigation() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let checked = Signal::new(false);
        let switched = Signal::new(false);
        let volume = Signal::new(0.5);
        let a = checked.clone();
        let b = switched.clone();
        let c = volume.clone();
        let root = RawView::new(creamui_widgets::layout::column(12.))
            .child(Box::new(Checkbox::new(false, move || a.set(true))))
            .child(Box::new(creamui_widgets::Switch::new(false, move || {
                b.set(true)
            })))
            .child(Box::new(Slider::new(0.5, move |v| c.set(v))));
        let mut painter = RecordingPainter::default();
        let scene = render_frame(
            Box::new(root),
            Size {
                width: 320.,
                height: 200.,
            },
            &mut painter,
        );
        assert_eq!(scene.next_focus(Some(0), true), Some(2));
        for index in [0, 1] {
            scene.on_key_at(index).unwrap()(KeyInput {
                key: Key::Char(' '),
                modifiers: Modifiers::default(),
            });
        }
        scene.on_key_at(2).unwrap()(KeyInput {
            key: Key::End,
            modifiers: Modifiers::default(),
        });
        assert!(checked.get() && switched.get());
        assert_eq!(volume.get(), 1.);
    });
}

#[test]
fn tab_sizing_supports_content_equal_and_fill_widths() {
    let labels = ["Short", "A much longer tab"];
    let content = tab_styles(&labels, TabSizing::Content, 36.0, 10.0);
    assert_eq!(content[0].size.width, Dimension::Auto);

    let equal = tab_styles(&labels, TabSizing::Equal, 36.0, 10.0);
    assert_eq!(equal[0].size.width, equal[1].size.width);

    let fill = tab_styles(&labels, TabSizing::Fill, 36.0, 10.0);
    assert_eq!(fill[0].flex_grow, 1.0);
    assert_eq!(fill[0].flex_basis, Dimension::Length(0.0));
}

#[test]
fn tabs_preserve_the_callers_layout_style() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let style = creamui_widgets::layout::row(13.0);
        let tabs = Tabs::new(TabColors::dark(), style.clone());
        assert_eq!(tabs.style().gap, style.gap);
    });
}

#[test]
fn select_is_controlled_and_its_popup_options_are_clickable() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let controller = SelectController::default();
        let build = || {
            Box::new(Select::controlled(
                &["System", "Light", "Dark"],
                controller.clone(),
            )) as creamui_core::BoxedWidget
        };
        let size = Size {
            width: 320.0,
            height: 200.0,
        };
        let mut painter = RecordingPainter::default();
        let closed = render_frame(build(), size, &mut painter);
        closed.on_key_at(0).unwrap()(KeyInput {
            key: Key::Down,
            modifiers: Modifiers::default(),
        });
        assert_eq!(
            controller.selected(),
            1,
            "arrow keys select an adjacent option"
        );
        controller.select(0);
        closed.on_key_at(0).unwrap()(KeyInput {
            key: Key::Enter,
            modifiers: Modifiers::default(),
        });
        assert!(controller.is_open());

        painter.texts.clear();
        let open = render_frame(build(), size, &mut painter);
        assert!(painter.texts.iter().any(|text| text == "Dark"));
        // Popup starts at y=40; its second 34px option occupies roughly y=77..111.
        open.hit_test(Point { x: 20.0, y: 92.0 }).unwrap()();
        assert_eq!(
            controller.selected(),
            1,
            "clicking an option must not fall through to content behind it"
        );
        assert!(!controller.is_open());

        controller.set_open(true);
        let open = render_frame(build(), size, &mut painter);
        // The transparent portal layer sits behind the popup but above the
        // application, so a click elsewhere dismisses it.
        open.hit_test(Point { x: 300.0, y: 180.0 }).unwrap()();
        assert!(
            !controller.is_open(),
            "clicking outside an open select must dismiss its portal"
        );
    });
}

#[test]
fn picker_portals_dismiss_on_an_outside_click() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let date = DateTimeController::default();
        let color = ColorPickerController::default();
        date.set_open(true);
        color.set_open(true);
        let build = || {
            Box::new(
                RawView::new(creamui_widgets::layout::row(20.0))
                    .child(Box::new(DateTimePicker::controlled(&date)))
                    .child(Box::new(ColorPicker::controlled(
                        Color::rgb(12, 34, 56),
                        &color,
                        |_| {},
                    ))),
            ) as creamui_core::BoxedWidget
        };
        let mut painter = RecordingPainter::default();
        let scene = render_frame(
            build(),
            Size {
                width: 700.0,
                height: 420.0,
            },
            &mut painter,
        );
        // Both popups are at the left of their fields; this point is in
        // neither popup and must reach the topmost dismiss layer.
        scene.hit_test(Point { x: 650.0, y: 400.0 }).unwrap()();
        assert!(!color.is_open());
        // The later color picker was topmost here, so rebuild and dismiss
        // the remaining date picker on a subsequent outside click.
        let scene = render_frame(
            build(),
            Size {
                width: 700.0,
                height: 420.0,
            },
            &mut painter,
        );
        scene.hit_test(Point { x: 650.0, y: 400.0 }).unwrap()();
        assert!(!date.is_open());
    });
}

#[test]
fn overlay_dismisses_outside_but_popover_shields_inside_clicks() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let dismisses = Signal::new(0);
        let count = dismisses.clone();
        let popover_style = Style {
            size: creamui_widgets::layout::fixed(100.0, 50.0),
            ..Default::default()
        };
        let root = Overlay::fullscreen(move || dismisses.update(|n| *n += 1))
            .child(Box::new(Popover::new(popover_style)));
        let mut painter = RecordingPainter::default();
        let scene = render_frame(
            Box::new(root),
            Size {
                width: 300.0,
                height: 200.0,
            },
            &mut painter,
        );
        scene.hit_test(Point { x: 10.0, y: 10.0 }).unwrap()();
        assert_eq!(count.get(), 1);
        // The centered popover consumes this click instead of dismissing.
        scene.hit_test(Point { x: 150.0, y: 100.0 }).unwrap()();
        assert_eq!(count.get(), 1);
    });
}

#[test]
fn progress_indicators_paint_accent_fill() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let root = RawView::new(creamui_widgets::layout::row(10.0))
            .child(Box::new(ProgressBar::new(0.5)))
            .child(Box::new(ProgressRing::new(0.5)));
        let mut painter = RecordingPainter::default();
        render_frame(
            Box::new(root),
            Size {
                width: 320.0,
                height: 80.0,
            },
            &mut painter,
        );
        assert!(painter
            .filled_rects
            .iter()
            .any(|(_, color)| *color == theme.accent));
    });
}

#[test]
fn raw_controls_preserve_tokens_and_disabled_state() {
    let style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(28.0),
            height: Dimension::Length(28.0),
        },
        ..Default::default()
    };
    let checkbox = RawCheckbox::new(18.0, false, Color::rgb(1, 2, 3), Color::rgb(4, 5, 6), || {})
        .layout_style(style.clone())
        .background(Color::rgb(7, 8, 9))
        .disabled(true);
    assert_eq!(checkbox.style().size, style.size);
    assert!(!checkbox.focusable());
    assert!(checkbox.on_click().is_none());

    let switch = RawSwitch::new(
        false,
        Color::rgb(1, 2, 3),
        Color::rgb(4, 5, 6),
        Color::rgb(255, 255, 255),
        || {},
    )
    .layout_style(style.clone())
    .disabled(true);
    assert_eq!(switch.style().size, style.size);
    assert!(switch.on_key().is_none());

    let slider = RawSlider::new(
        style,
        0.5,
        Color::rgb(0, 0, 0),
        Color::rgb(255, 255, 255),
        Color::rgb(255, 255, 255),
        |_| {},
    )
    .track(6.0, 3.0)
    .handle(20.0, 4.0)
    .disabled(true);
    assert_eq!(slider.track_height, 6.0);
    assert_eq!(slider.handle_size, 20.0);
    assert!(slider.on_drag().is_none());
}

#[test]
fn controlled_scroll_only_handles_wheel_when_content_overflows_and_clamps() {
    let viewport_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(120.0),
            height: Dimension::Length(100.0),
        },
        ..Default::default()
    };
    let controller = ScrollController::default();
    let overflowing = RawScrollView::controlled(viewport_style.clone(), controller.clone()).child(
        Box::new(RawView::new(Style {
            size: creamui_core::layout::Size {
                width: Dimension::Length(120.0),
                height: Dimension::Length(260.0),
            },
            ..Default::default()
        })),
    );
    let mut painter = RecordingPainter::default();
    let scene = render_frame(
        Box::new(overflowing),
        Size {
            width: 120.0,
            height: 100.0,
        },
        &mut painter,
    );
    let handler = scene
        .scroll_hit_test(Point { x: 20.0, y: 20.0 })
        .and_then(|index| scene.on_scroll_at(index))
        .expect("overflowing content registers a wheel handler");
    handler(500.0);
    assert_eq!(controller.peek(), 160.0);

    let fitting = RawScrollView::controlled(viewport_style, ScrollController::default()).child(
        Box::new(RawView::new(Style {
            size: creamui_core::layout::Size {
                width: Dimension::Length(120.0),
                height: Dimension::Length(60.0),
            },
            ..Default::default()
        })),
    );
    let scene = render_frame(
        Box::new(fitting),
        Size {
            width: 120.0,
            height: 100.0,
        },
        &mut painter,
    );
    assert!(scene.scroll_hit_test(Point { x: 20.0, y: 20.0 }).is_none());
}

#[test]
fn scrollbar_thumb_is_draggable_and_tracks_the_wheel() {
    let viewport_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(120.0),
            height: Dimension::Length(100.0),
        },
        ..Default::default()
    };
    let controller = ScrollController::default();
    let overflowing = RawScrollView::controlled(viewport_style, controller.clone()).child(
        Box::new(RawView::new(Style {
            size: creamui_core::layout::Size {
                width: Dimension::Length(120.0),
                height: Dimension::Length(260.0),
            },
            ..Default::default()
        })),
    );
    let mut painter = RecordingPainter::default();
    let scene = render_frame(
        Box::new(overflowing),
        Size {
            width: 120.0,
            height: 100.0,
        },
        &mut painter,
    );

    // Content overflow is 260 - 100 = 160px, so the scrollbar's own
    // `on_content_overflow` report should have already reached the shared
    // controller by the time this frame finished painting.
    assert_eq!(controller.max_offset(), 160.0);

    // The scrollbar sits a couple of pixels in from the right edge; (113, 50)
    // lands inside its default 10px-wide track.
    let index = scene
        .drag_hit_test(Point { x: 113.0, y: 50.0 })
        .expect("scrollbar track should be draggable when content overflows");
    let (rect, handler) = scene
        .draggable_at(index)
        .expect("draggable index should still resolve this frame");

    // Dragging to the track's bottom should jump the controller to its max.
    handler(
        Point {
            x: 113.0 - rect.x,
            y: rect.height,
        },
        rect,
    );
    assert_eq!(controller.peek(), 160.0);

    // Dragging back to the top should return it to zero.
    handler(
        Point {
            x: 113.0 - rect.x,
            y: 0.0,
        },
        rect,
    );
    assert_eq!(controller.peek(), 0.0);
}

#[test]
fn button_paints_and_responds_to_clicks() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let counter = Signal::new(0);
        let counter_for_click = counter.clone();

        let root_style = Style {
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            size: creamui_core::layout::Size {
                width: creamui_core::layout::Dimension::Length(400.0),
                height: creamui_core::layout::Dimension::Length(300.0),
            },
            ..Default::default()
        };

        let root = RawView::new(root_style).child(Box::new(Button::new("Click me", move || {
            counter_for_click.update(|c| *c += 1);
        })));

        let mut painter = RecordingPainter::default();
        let scene = render_frame(
            Box::new(root),
            Size {
                width: 400.0,
                height: 300.0,
            },
            &mut painter,
        );

        assert_eq!(painter.texts, vec!["Click me".to_string()]);
        assert!(
            painter
                .filled_rects
                .iter()
                .any(|(_, color)| *color == theme.accent),
            "button should paint with the theme's accent color"
        );

        let button_rect = painter
            .filled_rects
            .iter()
            .find(|(_, color)| *color == theme.accent)
            .map(|(rect, _)| *rect)
            .expect("button rect");
        let center = Point {
            x: button_rect.x + button_rect.width / 2.0,
            y: button_rect.y + button_rect.height / 2.0,
        };

        let handler = scene
            .hit_test(center)
            .expect("click inside button should hit");
        handler();
        assert_eq!(counter.get(), 1);

        assert!(
            scene.hit_test(Point { x: 0.0, y: 0.0 }).is_none(),
            "clicking far outside the button should not hit anything"
        );
    });
}

/// Regression test: `themed::Text` wraps `raw::RawText`, and every trait
/// method it doesn't explicitly forward silently falls back to
/// `Widget`'s default — which, for `measure`, is "no intrinsic size".
/// `RawText` used directly (e.g. inside `Button`) always worked; this
/// specifically catches the themed wrapper forgetting to forward
/// `measure`, which collapsed every `Text` widget's layout box to zero
/// and visually stacked its glyphs one per line.
#[test]
fn themed_text_reports_a_real_intrinsic_width() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        // An auto-sized (hug-content) row: without a working `measure`, the
        // text node's layout box collapses to ~0 width.
        let root = RawView::new(creamui_widgets::layout::row(0.0))
            .child(Box::new(Text::new("Hello, CreamUI!").font_size(28.0)));

        let mut painter = RecordingPainter::default();
        render_frame(
            Box::new(root),
            Size {
                width: 800.0,
                height: 200.0,
            },
            &mut painter,
        );

        assert_eq!(painter.texts, vec!["Hello, CreamUI!".to_string()]);
        assert!(
            painter.text_rects[0].width > 100.0,
            "themed Text should measure a real width for a 28px heading, got {:?}",
            painter.text_rects[0]
        );
    });
}

#[test]
fn checkbox_toggles_on_click_and_repaints_accordingly() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let checked = Signal::new(false);

        let build = |checked: Signal<bool>| {
            let checked_for_click = checked.clone();
            let root_style = Style {
                justify_content: Some(JustifyContent::Center),
                align_items: Some(AlignItems::Center),
                size: creamui_core::layout::Size {
                    width: creamui_core::layout::Dimension::Length(200.0),
                    height: creamui_core::layout::Dimension::Length(200.0),
                },
                ..Default::default()
            };
            RawView::new(root_style).child(Box::new(Checkbox::new(checked.get(), move || {
                checked_for_click.update(|c| *c = !*c);
            })))
        };

        let mut painter = RecordingPainter::default();
        let size = Size {
            width: 200.0,
            height: 200.0,
        };
        let scene = render_frame(Box::new(build(checked.clone())), size, &mut painter);

        // Unchecked: painted as an outline, not a fill.
        assert!(painter
            .stroked_rects
            .iter()
            .any(|(_, color)| *color == theme.border_strong));
        assert!(!painter
            .filled_rects
            .iter()
            .any(|(_, color)| *color == theme.accent));

        let checkbox_rect = painter.stroked_rects[0].0;
        let center = Point {
            x: checkbox_rect.x + checkbox_rect.width / 2.0,
            y: checkbox_rect.y + checkbox_rect.height / 2.0,
        };
        scene
            .hit_test(center)
            .expect("click inside checkbox should hit")();
        assert!(checked.get(), "click should toggle the backing signal");

        // Re-render with the now-checked state: painted as a fill, not an outline.
        let mut painter2 = RecordingPainter::default();
        render_frame(Box::new(build(checked.clone())), size, &mut painter2);
        assert!(painter2
            .filled_rects
            .iter()
            .any(|(_, color)| *color == theme.accent));
    });
}

#[test]
fn text_input_is_focusable_and_types_and_deletes_characters() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let value = Signal::new(String::new());

        // Mirrors what `creamui_render::window` actually does: rebuild the tree
        // and fetch a fresh `on_key` from the new `Scene` after every keystroke,
        // since (like `Button`'s `on_click`) `on_key` closures capture the value
        // as of the render that produced them.
        let build = |value: Signal<String>| {
            let value_for_change = value.clone();
            RawView::new(creamui_widgets::layout::row(0.0))
                .child(Box::new(TextInput::new(value.get(), move |next| {
                    value_for_change.set(next)
                })))
        };
        let size = Size {
            width: 400.0,
            height: 100.0,
        };

        let mut painter = RecordingPainter::default();
        let scene = render_frame(Box::new(build(value.clone())), size, &mut painter);
        let input_rect = painter
            .stroked_rects
            .iter()
            .find(|(_, color)| *color == theme.border)
            .map(|(rect, _)| *rect)
            .expect("text input border rect");
        let center = Point {
            x: input_rect.x + input_rect.width / 2.0,
            y: input_rect.y + input_rect.height / 2.0,
        };
        let index = scene
            .focus_hit_test(center)
            .expect("text input should be focusable");
        assert!(
            scene.focus_hit_test(Point { x: 399.0, y: 99.0 }).is_none(),
            "a point far from the (top-left-positioned, 200x36) input should not be focusable"
        );

        scene.on_key_at(index).unwrap().clone()(KeyInput {
            key: Key::Char('h'),
            modifiers: Default::default(),
        });
        assert_eq!(value.get(), "h");

        let scene = render_frame(
            Box::new(build(value.clone())),
            size,
            &mut RecordingPainter::default(),
        );
        scene.on_key_at(index).unwrap().clone()(KeyInput {
            key: Key::Char('i'),
            modifiers: Default::default(),
        });
        assert_eq!(value.get(), "hi");

        let scene = render_frame(
            Box::new(build(value.clone())),
            size,
            &mut RecordingPainter::default(),
        );
        scene.on_key_at(index).unwrap().clone()(KeyInput {
            key: Key::Backspace,
            modifiers: Default::default(),
        });
        assert_eq!(value.get(), "h");
    });
}

#[test]
fn text_area_accepts_newlines_and_backspace() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let value = Signal::new(String::from("first"));
        let build = |value: Signal<String>| {
            let next_value = value.clone();
            RawView::new(creamui_widgets::layout::row(0.0))
                .child(Box::new(TextArea::new(value.get(), move |next| {
                    next_value.set(next)
                })))
        };
        let size = Size {
            width: 500.0,
            height: 300.0,
        };
        let scene = render_frame(
            Box::new(build(value.clone())),
            size,
            &mut RecordingPainter::default(),
        );
        let index = scene
            .focus_hit_test(Point { x: 30.0, y: 30.0 })
            .expect("text area should be focusable");
        scene.on_key_at(index).unwrap().clone()(KeyInput {
            key: Key::Enter,
            modifiers: Default::default(),
        });
        assert_eq!(value.get(), "first\n");
        let scene = render_frame(
            Box::new(build(value.clone())),
            size,
            &mut RecordingPainter::default(),
        );
        scene.on_key_at(index).unwrap().clone()(KeyInput {
            key: Key::Char('x'),
            modifiers: Default::default(),
        });
        assert_eq!(value.get(), "first\nx");
        let scene = render_frame(
            Box::new(build(value.clone())),
            size,
            &mut RecordingPainter::default(),
        );
        scene.on_key_at(index).unwrap().clone()(KeyInput {
            key: Key::Backspace,
            modifiers: Default::default(),
        });
        assert_eq!(value.get(), "first\n");
    });
}

#[test]
fn text_area_drag_and_shift_arrows_update_controlled_selection() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let value = Signal::new(String::from("first\nsecond"));
        let cursor = Signal::new(0usize);
        let selection = Signal::new(TextSelection::default());
        let build =
            |value: Signal<String>, cursor: Signal<usize>, selection: Signal<TextSelection>| {
                let value_for_change = value.clone();
                let cursor_for_change = cursor.clone();
                let selection_for_change = selection.clone();
                RawView::new(creamui_widgets::layout::row(0.0)).child(Box::new(
                    TextArea::new(value.get(), move |next| value_for_change.set(next))
                        .cursor(cursor.get(), move |next| cursor_for_change.set(next))
                        .selection(selection.get(), move |next| selection_for_change.set(next))
                        .selection_background(Color::rgb(0x20, 0x55, 0x88)),
                ))
            };
        let size = Size {
            width: 500.0,
            height: 300.0,
        };
        let scene = render_frame(
            Box::new(build(value.clone(), cursor.clone(), selection.clone())),
            size,
            &mut RecordingPainter::default(),
        );
        let (_, start) = scene
            .drag_start_at(Point { x: 14.0, y: 14.0 })
            .expect("textarea drag start");
        start(
            Point { x: 14.0, y: 14.0 },
            Rect {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 240.0,
            },
        );
        let scene = render_frame(
            Box::new(build(value.clone(), cursor.clone(), selection.clone())),
            size,
            &mut RecordingPainter::default(),
        );
        let index = scene
            .drag_hit_test(Point { x: 48.0, y: 14.0 })
            .expect("textarea draggable");
        let (rect, drag) = scene.draggable_at(index).unwrap();
        drag.clone()(
            Point {
                x: 48.0 - rect.x,
                y: 14.0 - rect.y,
            },
            rect,
        );
        assert!(
            selection.get().focus > selection.get().anchor,
            "drag should extend selection: {:?}",
            selection.get()
        );

        let scene = render_frame(
            Box::new(build(value.clone(), cursor.clone(), selection.clone())),
            size,
            &mut RecordingPainter::default(),
        );
        let focused = scene.focus_hit_test(Point { x: 30.0, y: 30.0 }).unwrap();
        scene.on_key_at(focused).unwrap().clone()(KeyInput {
            key: Key::Right,
            modifiers: Modifiers {
                ctrl: false,
                shift: true,
            },
        });
        assert!(
            !selection.get().is_empty(),
            "Shift+Right should retain a selection"
        );
    });
}

#[test]
fn text_area_ctrl_a_selects_the_entire_controlled_document() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let value = Signal::new(String::from("select all"));
        let cursor = Signal::new(0usize);
        let selection = Signal::new(TextSelection::default());
        let value_for_change = value.clone();
        let cursor_for_change = cursor.clone();
        let selection_for_change = selection.clone();
        let root = RawView::new(creamui_widgets::layout::row(0.0)).child(Box::new(
            TextArea::new(value.get(), move |next| value_for_change.set(next))
                .cursor(cursor.get(), move |next| cursor_for_change.set(next))
                .selection(selection.get(), move |next| selection_for_change.set(next)),
        ));
        let scene = render_frame(
            Box::new(root),
            Size {
                width: 500.0,
                height: 300.0,
            },
            &mut RecordingPainter::default(),
        );
        let index = scene.focus_hit_test(Point { x: 20.0, y: 20.0 }).unwrap();
        scene.on_key_at(index).unwrap().clone()(KeyInput {
            key: Key::Char('a'),
            modifiers: Modifiers {
                ctrl: true,
                shift: false,
            },
        });
        assert_eq!(
            selection.get(),
            TextSelection {
                anchor: 0,
                focus: value.get().len()
            }
        );
    });
}

#[test]
fn text_area_paints_each_source_line_at_its_own_baseline() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let root = RawView::new(creamui_widgets::layout::row(0.0))
            .child(Box::new(TextArea::new("first\nsecond", |_| {})));
        let mut painter = RecordingPainter::default();
        render_frame(
            Box::new(root),
            Size {
                width: 500.0,
                height: 300.0,
            },
            &mut painter,
        );
        assert!(painter.texts.iter().any(|text| text == "first"));
        assert!(painter.texts.iter().any(|text| text == "second"));
        assert!(
            !painter.texts.iter().any(|text| text == "first\nsecond"),
            "a textarea must not hand the painter one vertically-centered document block"
        );
    });
}

#[test]
fn text_input_shows_a_hover_cursor_and_a_focus_only_blinking_caret() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let value = Signal::new(String::from("hi"));
        let build = |value: Signal<String>| {
            RawView::new(creamui_widgets::layout::row(0.0))
                .child(Box::new(TextInput::new(value.get(), move |_| {})))
        };
        let size = Size {
            width: 400.0,
            height: 100.0,
        };

        let mut renderer = Renderer::new();
        let mut painter = RecordingPainter::default();
        let scene = renderer.render(Box::new(build(value.clone())), size, &mut painter);

        let input_rect = painter
            .stroked_rects
            .iter()
            .find(|(_, color)| *color == theme.border)
            .map(|(rect, _)| *rect)
            .expect("text input border rect");
        let center = Point {
            x: input_rect.x + input_rect.width / 2.0,
            y: input_rect.y + input_rect.height / 2.0,
        };
        let index = scene
            .focus_hit_test(center)
            .expect("text input should be focusable");

        assert_eq!(
            scene.cursor_hit_test(center),
            Some(CursorIcon::Text),
            "hovering a text input should request the system text (I-beam) cursor"
        );

        // A thin (sub-2px) filled rect is the caret; nothing else this widget
        // paints is that narrow.
        let is_caret = |(rect, _): &&(Rect, Color)| rect.width < 2.0;

        let unfocused_carets = painter.filled_rects.iter().filter(is_caret).count();
        assert_eq!(
            unfocused_carets, 0,
            "an unfocused text input should not paint a caret"
        );

        let mut focused_painter = RecordingPainter::default();
        renderer.render_focused(
            Box::new(build(value.clone())),
            size,
            &mut focused_painter,
            Some(index),
            true,
        );
        let visible_carets = focused_painter.filled_rects.iter().filter(is_caret).count();
        assert_eq!(
            visible_carets, 1,
            "a focused text input should paint exactly one caret rect while blinked on"
        );

        let mut blink_off_painter = RecordingPainter::default();
        renderer.render_focused(
            Box::new(build(value.clone())),
            size,
            &mut blink_off_painter,
            Some(index),
            false,
        );
        let blinked_off_carets = blink_off_painter
            .filled_rects
            .iter()
            .filter(is_caret)
            .count();
        assert_eq!(
            blinked_off_carets, 0,
            "the caret should disappear during the off phase of its blink"
        );
    });
}

#[test]
fn slider_drag_updates_value_proportionally() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let value = Signal::new(0.0_f32);
        let value_for_change = value.clone();

        let root = RawView::new(creamui_widgets::layout::row(0.0))
            .child(Box::new(Slider::new(value.get(), move |next| {
                value_for_change.set(next)
            })));

        let mut painter = RecordingPainter::default();
        let size = Size {
            width: 400.0,
            height: 100.0,
        };
        let scene = render_frame(Box::new(root), size, &mut painter);

        let slider_rect = painter
            .filled_rects
            .iter()
            .find(|(_, color)| *color == theme.border_strong)
            .map(|(rect, _)| *rect)
            .expect("slider track rect");
        let center = Point {
            x: slider_rect.x + slider_rect.width / 2.0,
            y: slider_rect.y + slider_rect.height / 2.0,
        };

        let index = scene
            .drag_hit_test(center)
            .expect("slider should be draggable");
        let (rect, on_drag) = scene.draggable_at(index).expect("draggable slider region");
        let local = Point {
            x: rect.width / 2.0,
            y: rect.height / 2.0,
        };
        on_drag(local, rect);

        assert!(
            (value.get() - 0.5).abs() < 0.05,
            "dragging to the middle of the track should set roughly 0.5, got {}",
            value.get()
        );
    });
}

fn fixed_size_button(width: f32, height: f32, on_click: impl Fn() + 'static) -> RawButton {
    let style = Style {
        size: creamui_core::layout::Size {
            width: creamui_core::layout::Dimension::Length(width),
            height: creamui_core::layout::Dimension::Length(height),
        },
        ..Default::default()
    };
    RawButton::new(style, on_click)
}

#[test]
fn scroll_view_clips_hit_testing_to_its_visible_area_and_scrolls() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let clicked = Signal::new(-1i32);
        let scroll_y = Signal::new(0.0_f32);

        // Container viewport is 60px tall; three 40px-tall buttons stacked with
        // no gap total 120px of content, so button 2 (y=[80,120)) starts out
        // fully below the visible area.
        let build = |scroll_y: Signal<f32>| {
            let scroll_y_for_scroll = scroll_y.clone();
            let mut view = ScrollView::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: creamui_core::layout::Dimension::Length(80.0),
                        height: creamui_core::layout::Dimension::Length(60.0),
                    },
                    ..Default::default()
                },
                scroll_y.get(),
                move |delta| scroll_y_for_scroll.update(|y| *y = (*y + delta).clamp(0.0, 60.0)),
            );
            for i in 0..3 {
                let clicked = clicked.clone();
                view = view.child(Box::new(fixed_size_button(80.0, 40.0, move || {
                    clicked.set(i)
                })));
            }
            view
        };

        let size = Size {
            width: 200.0,
            height: 200.0,
        };
        let mut painter = RecordingPainter::default();
        let scene = render_frame(Box::new(build(scroll_y.clone())), size, &mut painter);

        // Button 2's natural (unclipped) position is y=[80,120); at scroll_y=0
        // that's entirely outside the 60px-tall viewport, so no click should
        // land there at all.
        let point_below_viewport = Point { x: 40.0, y: 100.0 };
        assert!(
            scene.hit_test(point_below_viewport).is_none(),
            "content scrolled out of view should not be clickable"
        );

        // Button 0 (y=[0,40)) is fully visible at scroll_y=0 and should still work normally.
        scene
            .hit_test(Point { x: 40.0, y: 20.0 })
            .expect("button 0 should be visible and clickable")();
        assert_eq!(clicked.get(), 0);

        // Simulate the scroll wheel: hit-test for a scrollable at a point inside
        // the view, then drive it the same way `creamui_render::window` does.
        let scroll_index = scene
            .scroll_hit_test(Point { x: 40.0, y: 30.0 })
            .expect("scroll view should be scrollable");
        scene.on_scroll_at(scroll_index).unwrap()(60.0);
        assert_eq!(
            scroll_y.get(),
            60.0,
            "scrolling by 60px should move the clamped offset to 60"
        );

        // Re-render with the new offset: button 2 is now shifted up into view
        // (screen y = 80 - 60 = 20, i.e. within [0, 40)), so the same point that
        // missed before should now hit button 2.
        let scene = render_frame(
            Box::new(build(scroll_y.clone())),
            size,
            &mut RecordingPainter::default(),
        );
        scene
            .hit_test(Point { x: 40.0, y: 20.0 })
            .expect("button 2 should have scrolled into view")();
        assert_eq!(clicked.get(), 2);
    });
}

#[test]
fn list_box_clicking_a_row_reports_its_index() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let selected = Signal::new(0usize);
        let build = |selected: Signal<usize>| {
            let select = selected.clone();
            ListBox::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(160.0),
                        height: Dimension::Length(100.0),
                    },
                    ..Default::default()
                },
                ScrollController::default(),
                selected.get(),
                move |index| select.set(index),
            )
            .options(&["Alpha", "Bravo", "Charlie"])
            .row_height(32.0)
        };
        let scene = render_frame(
            Box::new(build(selected.clone())),
            Size {
                width: 160.0,
                height: 100.0,
            },
            &mut RecordingPainter::default(),
        );
        scene
            .hit_test(Point { x: 20.0, y: 40.0 })
            .expect("second row should be clickable")();
        assert_eq!(selected.get(), 1);
    });
}

#[test]
fn tree_view_hides_collapsed_children_until_toggled_open() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let controller = TreeController::default();
        let nodes = vec![TreeNode::new(1, "Documents").with_children(vec![
            TreeNode::new(2, "Resume.pdf"),
            TreeNode::new(3, "Notes.txt"),
        ])];
        let viewport_style = Style {
            size: creamui_core::layout::Size {
                width: Dimension::Length(160.0),
                height: Dimension::Length(120.0),
            },
            ..Default::default()
        };

        let collapsed = TreeView::new(
            viewport_style.clone(),
            ScrollController::default(),
            controller.clone(),
            &nodes,
        )
        .row_height(30.0);
        let scene = render_frame(
            Box::new(collapsed),
            Size {
                width: 160.0,
                height: 120.0,
            },
            &mut RecordingPainter::default(),
        );
        // Only the root row exists; a click where the second child would be
        // (row index 1, y in [30, 60)) should hit nothing.
        assert!(scene.hit_test(Point { x: 100.0, y: 45.0 }).is_none());
        // Clicking the root's chevron (near its left edge) expands it.
        scene
            .hit_test(Point { x: 10.0, y: 10.0 })
            .expect("root chevron should be clickable")();
        assert!(controller.is_expanded(1));

        let expanded = TreeView::new(
            viewport_style,
            ScrollController::default(),
            controller.clone(),
            &nodes,
        )
        .row_height(30.0);
        let scene = render_frame(
            Box::new(expanded),
            Size {
                width: 160.0,
                height: 120.0,
            },
            &mut RecordingPainter::default(),
        );
        // Now the second child row (y in [60, 90)) should be selectable.
        scene
            .hit_test(Point { x: 100.0, y: 75.0 })
            .expect("second child row should be visible after expanding")();
        assert_eq!(controller.peek_selected(), Some(3));
    });
}

#[test]
fn list_view_stacks_arbitrary_rows_and_scrolls_like_raw_scroll_view() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let clicked = Signal::new(-1i32);
        let scroll = ScrollController::default();
        let row_style = Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(40.0),
            },
            flex_shrink: 0.0,
            ..Default::default()
        };
        let build = |scroll: ScrollController, clicked: Signal<i32>| {
            let mut list = ListView::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(120.0),
                        height: Dimension::Length(60.0),
                    },
                    ..Default::default()
                },
                scroll,
            );
            for i in 0..3 {
                let record = clicked.clone();
                list = list.row(Box::new(RawButton::new(row_style.clone(), move || {
                    record.set(i)
                })));
            }
            list
        };
        let size = Size {
            width: 120.0,
            height: 60.0,
        };
        let mut painter = RecordingPainter::default();
        let scene = render_frame(
            Box::new(build(scroll.clone(), clicked.clone())),
            size,
            &mut painter,
        );
        scene
            .hit_test(Point { x: 10.0, y: 10.0 })
            .expect("first row is visible")();
        assert_eq!(clicked.get(), 0);
        assert!(
            scene.hit_test(Point { x: 10.0, y: 90.0 }).is_none(),
            "third row starts past the 60px viewport and should not be clickable yet"
        );

        let scroll_index = scene
            .scroll_hit_test(Point { x: 10.0, y: 10.0 })
            .expect("list view should be scrollable");
        scene.on_scroll_at(scroll_index).unwrap()(80.0);

        let scene = render_frame(
            Box::new(build(scroll.clone(), clicked.clone())),
            size,
            &mut painter,
        );
        // Content is 3*40px rows plus two 1px dividers = 122px over a 60px
        // viewport, so scrolling clamps to a 62px max offset rather than the
        // requested 80 — enough to bring the third row (content y in [82, 122))
        // into view, just not flush with the top.
        scene
            .hit_test(Point { x: 10.0, y: 30.0 })
            .expect("third row should have scrolled into view")();
        assert_eq!(clicked.get(), 2);
    });
}

#[test]
fn table_renders_header_labels_and_clicking_a_row_reports_its_index() {
    let theme = Theme::dark();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(theme));
        let selected = Signal::new(0usize);
        let columns = vec![
            TableColumn::new("Name", 80.0),
            TableColumn::new("Score", 60.0),
        ];
        let build = |selected: Signal<usize>| {
            let set_selected = selected.clone();
            Table::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(140.0),
                        height: Dimension::Length(120.0),
                    },
                    ..Default::default()
                },
                ScrollController::default(),
                columns.clone(),
            )
            .row(vec!["Ada".to_owned(), "97".to_owned()])
            .row(vec!["Grace".to_owned(), "95".to_owned()])
            .row(vec!["Alan".to_owned(), "99".to_owned()])
            .on_row_click(Some(selected.get()), move |index| set_selected.set(index))
        };
        let mut painter = RecordingPainter::default();
        let scene = render_frame(
            Box::new(build(selected.clone())),
            Size {
                width: 140.0,
                height: 120.0,
            },
            &mut painter,
        );
        assert!(painter.texts.iter().any(|t| t == "Name"));
        assert!(painter.texts.iter().any(|t| t == "Score"));
        assert!(painter.texts.iter().any(|t| t == "Grace"));

        // Header is 32px tall, each row 28px: the second row ("Grace") sits at
        // roughly y in [60, 88).
        scene
            .hit_test(Point { x: 20.0, y: 70.0 })
            .expect("second row should be clickable")();
        assert_eq!(selected.get(), 1);
    });
}
