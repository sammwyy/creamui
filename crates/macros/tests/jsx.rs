use creamui_core::layout::{FlexDirection, Style};
use creamui_core::{
    render_frame, BoxedWidget, Painter, Rect, Size, StateStyle, Style as CommonStyle, TextAlign,
    Widget,
};
use creamui_macros::{abi_jsx, component, jsx};
use creamui_reactive::Signal;
use creamui_theme::{Color, Theme};
use creamui_widgets::layout::{Align, Justify, Track, Wrap};

#[derive(Default)]
struct TextPainter(Vec<String>);

#[derive(Default)]
struct BoundsPainter(Vec<Rect>);

#[derive(Default)]
struct BorderPainter {
    backgrounds: Vec<Rect>,
    strokes: Vec<(Rect, f32)>,
}

#[derive(Default)]
struct TextBoxPainter {
    backgrounds: Vec<Rect>,
    text: Vec<(Rect, Color)>,
}

impl Painter for TextBoxPainter {
    fn fill_rect(&mut self, rect: Rect, _: Color, _: f32) {
        self.backgrounds.push(rect);
    }

    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}

    fn fill_text(&mut self, rect: Rect, _: &str, color: Color, _: f32, _: TextAlign) {
        self.text.push((rect, color));
    }
}

#[test]
fn text_uses_the_layout_content_box_and_explicit_color() {
    let widget = jsx! {
        <RawText width={100.0} height={50.0} padding={"10px 20px"}
            border={(Color::rgb(0, 0, 255), 4.0)}
            background={Color::rgb(255, 255, 255)} color={Color::rgb(255, 0, 0)}>
            "Inside"
        </RawText>
    };
    let mut painter = TextBoxPainter::default();
    render_frame(
        Box::new(widget),
        Size {
            width: 300.0,
            height: 200.0,
        },
        &mut painter,
    );
    assert_eq!(
        painter.backgrounds[0],
        Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0
        }
    );
    assert_eq!(
        painter.text[0],
        (
            Rect {
                x: 24.0,
                y: 14.0,
                width: 52.0,
                height: 22.0
            },
            Color::rgb(255, 0, 0)
        )
    );
}

impl Painter for BorderPainter {
    fn fill_rect(&mut self, rect: Rect, _: Color, _: f32) {
        self.backgrounds.push(rect);
    }

    fn stroke_rect(&mut self, rect: Rect, _: Color, width: f32, _: f32) {
        self.strokes.push((rect, width));
    }

    fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
}

#[test]
fn borders_reserve_space_and_paint_inside_both_box_models() {
    use creamui_core::layout::BoxSizing;
    use creamui_widgets::raw::RawView;

    for (sizing, outer_width, outer_height) in [
        (BoxSizing::BorderBox, 80.0, 40.0),
        (BoxSizing::ContentBox, 108.0, 68.0),
    ] {
        let root = jsx! {
            <RawView width={80.0} height={40.0} padding={10.0} box_sizing={sizing}
                border={(Color::rgb(0, 0, 255), 4.0)} background={Color::rgb(1, 2, 3)}>
                <RawView width={10.0} height={10.0} background={Color::rgb(4, 5, 6)} />
            </RawView>
        };
        let mut painter = BorderPainter::default();
        render_frame(
            Box::new(root),
            Size {
                width: 300.0,
                height: 200.0,
            },
            &mut painter,
        );
        assert_eq!(painter.backgrounds[0].width, outer_width);
        assert_eq!(painter.backgrounds[0].height, outer_height);
        assert_eq!(
            (painter.backgrounds[1].x, painter.backgrounds[1].y),
            (14.0, 14.0)
        );
        assert_eq!(
            painter.strokes,
            [(
                Rect {
                    x: 2.0,
                    y: 2.0,
                    width: outer_width - 4.0,
                    height: outer_height - 4.0
                },
                4.0
            )]
        );
    }

    let root = RawView::new(
        CommonStyle::new()
            .width(80.0)
            .height(40.0)
            .padding(10.0)
            .border(Color::rgb(0, 0, 255), 2.0)
            .background(Color::rgb(1, 2, 3))
            .focus(StateStyle::new().border(Color::rgb(0, 0, 255), 6.0)),
    )
    .child(Box::new(RawView::new(
        CommonStyle::new()
            .width(10.0)
            .height(10.0)
            .background(Color::rgb(4, 5, 6)),
    )));
    let mut painter = BorderPainter::default();
    render_frame(
        Box::new(root),
        Size {
            width: 300.0,
            height: 200.0,
        },
        &mut painter,
    );
    assert_eq!(
        (painter.backgrounds[0].width, painter.backgrounds[0].height),
        (80.0, 40.0)
    );
    assert_eq!(
        (painter.backgrounds[1].x, painter.backgrounds[1].y),
        (16.0, 16.0)
    );
}

impl Painter for BoundsPainter {
    fn fill_rect(&mut self, rect: Rect, _: Color, _: f32) {
        self.0.push(rect);
    }
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
    fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
}

fn painted_bounds(widget: impl Widget + 'static) -> Vec<Rect> {
    let mut painter = BoundsPainter::default();
    render_frame(
        Box::new(widget),
        Size {
            width: 640.0,
            height: 480.0,
        },
        &mut painter,
    );
    painter.0
}

#[test]
fn aspect_ratio_sizes_native_and_jsx_widgets() {
    use creamui_core::Styled;
    use creamui_widgets::raw::RawView;

    let native = RawView::new(CommonStyle::new())
        .width(160.0)
        .aspect_ratio(16.0 / 9.0)
        .background(Color::rgb(1, 2, 3));
    let jsx = jsx! {
        <RawView width={160.0} aspect_ratio={16.0 / 9.0} background={Color::rgb(1, 2, 3)} />
    };
    let bounds = painted_bounds(native);
    assert_eq!(bounds, painted_bounds(jsx));
    assert_eq!(bounds[0].width, 160.0);
    assert_eq!(bounds[0].height, 90.0);

    let explicit = jsx! {
        <RawView width={160.0} height={40.0} aspect_ratio={2.0} background={Color::rgb(1, 2, 3)} />
    };
    assert_eq!(painted_bounds(explicit)[0].height, 40.0);
}

#[test]
fn box_sizing_controls_padding_in_native_and_jsx_layout() {
    use creamui_core::layout::BoxSizing;
    use creamui_widgets::raw::RawView;

    for (sizing, width, height) in [
        (BoxSizing::BorderBox, 80.0, 40.0),
        (BoxSizing::ContentBox, 100.0, 60.0),
    ] {
        let native = RawView::new(
            CommonStyle::new()
                .width(80.0)
                .height(40.0)
                .padding(10.0)
                .box_sizing(sizing)
                .background(Color::rgb(1, 2, 3)),
        );
        let jsx = jsx! {
            <RawView width={80.0} height={40.0} padding={10.0}
                box_sizing={sizing} background={Color::rgb(1, 2, 3)} />
        };
        let bounds = painted_bounds(native);
        assert_eq!(bounds, painted_bounds(jsx));
        assert_eq!((bounds[0].width, bounds[0].height), (width, height));
    }
}

#[test]
fn spacing_shorthands_position_native_and_jsx_children() {
    use creamui_widgets::raw::RawView;

    let native = RawView::new(
        CommonStyle::new()
            .width(200.0)
            .height(100.0)
            .padding("10px 20px 30px 40px"),
    )
    .child(Box::new(RawView::new(
        CommonStyle::new()
            .width(20.0)
            .height(10.0)
            .margin("5px 0")
            .background(Color::rgb(1, 2, 3)),
    )));
    let jsx = jsx! {
        <RawView width={200.0} height={100.0} padding={"10px 20px 30px 40px"}>
            <RawView width={20.0} height={10.0} margin={"5px 0"} background={Color::rgb(1, 2, 3)} />
        </RawView>
    };
    let bounds = painted_bounds(native);
    assert_eq!(bounds, painted_bounds(jsx));
    assert_eq!(
        bounds[0],
        Rect {
            x: 40.0,
            y: 15.0,
            width: 20.0,
            height: 10.0
        }
    );

    let overlay = jsx! {
        <RawView width={200.0} height={100.0}>
            <RawView position={creamui_core::layout::Position::Absolute}
                inset={"10px 20px 30px 40px"} background={Color::rgb(1, 2, 3)} />
        </RawView>
    };
    assert_eq!(
        painted_bounds(overlay)[0],
        Rect {
            x: 40.0,
            y: 10.0,
            width: 140.0,
            height: 60.0
        }
    );
}

#[test]
fn spacing_shorthands_work_on_semantic_layout_containers() {
    use creamui_core::layout::{LengthPercentage, LengthPercentageAuto};

    let block = jsx! { <Block padding={"8px 16px"} margin={"0 auto"} /> };
    let flex = jsx! { <Flex padding={"8px 16px"} margin={"0 auto"} /> };
    let grid = jsx! { <Grid padding={"8px 16px"} margin={"0 auto"} /> };
    for widget in [
        &block as &dyn Widget,
        &flex as &dyn Widget,
        &grid as &dyn Widget,
    ] {
        let layout = widget.style().layout;
        assert_eq!(layout.padding.top, LengthPercentage::Length(8.0));
        assert_eq!(layout.padding.left, LengthPercentage::Length(16.0));
        assert_eq!(layout.margin.top, LengthPercentageAuto::Length(0.0));
        assert_eq!(layout.margin.left, LengthPercentageAuto::Auto);
    }
}

#[test]
fn radial_gradients_record_the_same_paint_from_native_and_jsx() {
    use creamui_core::{runtime::RecordingPainter, RadialGradient};
    use creamui_widgets::raw::RawView;

    let native = RawView::new(
        CommonStyle::new()
            .width(100.0)
            .height(80.0)
            .corner_radius(6.0)
            .background(
                RadialGradient::new(Color::rgb(255, 0, 0), Color::rgb(0, 0, 255)).at(0.25, 0.75),
            ),
    );
    let jsx = jsx! {
        <RawView width={100.0} height={80.0} corner_radius={6.0}
            background={"radial-gradient(circle at 25% 75%, #ff0000, #0000ff)"} />
    };
    let record = |widget: BoxedWidget| {
        let mut painter = RecordingPainter::new(Default::default());
        render_frame(
            widget,
            Size {
                width: 200.0,
                height: 200.0,
            },
            &mut painter,
        );
        painter.into_ops()
    };
    let ops = record(Box::new(native));
    assert_eq!(ops, record(Box::new(jsx)));
    let gradient = ops
        .iter()
        .find_map(|op| match op {
            creamui_core::runtime::PaintOp::Primitive(
                creamui_core::runtime::PaintPrimitive::RadialGradient(gradient),
            ) => Some(gradient),
            _ => None,
        })
        .unwrap();
    assert_eq!(gradient.center, creamui_core::Point { x: 25.0, y: 60.0 });
    assert_eq!(gradient.radius, 75.0_f32.hypot(60.0));
    assert_eq!(gradient.corner_radius, 6.0);
}

#[component]
fn CounterLabel(value: i32) -> BoxedWidget {
    Box::new(jsx! { <Text>{format!("Custom: {value}")}</Text> })
}

#[component]
fn Panel(children: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(creamui_widgets::raw::RawView::new(Style::default()).with_children(children))
}

#[component]
fn AbiLabel(
    ctx: creamui_dynamic::Context,
    theme: creamui_dynamic::Theme,
    label: String,
) -> creamui_dynamic::Widget {
    abi_jsx! { <Text ctx={&ctx} theme={theme}>{label}</Text> }
}

#[allow(dead_code)]
fn build_abi_component(
    ctx: &creamui_dynamic::Context,
    theme: creamui_dynamic::Theme,
) -> creamui_dynamic::Widget {
    abi_jsx! { <AbiLabel ctx={ctx.clone()} theme={theme} label={"from ABI".to_string()} /> }
}

impl Painter for TextPainter {
    fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
    fn fill_text(&mut self, _: Rect, text: &str, _: Color, _: f32, _: TextAlign) {
        self.0.push(text.into());
    }
}

#[test]
fn jsx_expands_to_the_existing_widget_builders() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(Theme::dark()));
        let clicks = Signal::new(0);
        let clicks_for_handler = clicks.clone();
        let root: BoxedWidget = Box::new(jsx! {
            <Block style={Style::default()}>
                <Text font_size={20.0}>{format!("Clicked {} times", clicks.get())}</Text>
                <Button on_click={move || clicks_for_handler.update(|value| *value += 1)}>"Increment"</Button>
            </Block>
        });

        let mut painter = TextPainter::default();
        let scene = render_frame(
            root,
            Size {
                width: 300.0,
                height: 120.0,
            },
            &mut painter,
        );
        assert_eq!(painter.0, ["Clicked 0 times", "Increment"]);
        let click = (0..300)
            .step_by(4)
            .flat_map(|x| (0..120).step_by(4).map(move |y| (x, y)))
            .find_map(|(x, y)| {
                scene.hit_test(creamui_core::Point {
                    x: x as f32,
                    y: y as f32,
                })
            })
            .expect("button hit");
        click();
        assert_eq!(clicks.get(), 1);
    });
}

#[test]
fn application_components_are_typed_functions_not_macro_registrations() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(Theme::dark()));
        let root: BoxedWidget = Box::new(jsx! {
            <Block style={Style::default()}>
                <CounterLabel value={7} />
            </Block>
        });
        let mut painter = TextPainter::default();
        render_frame(
            root,
            Size {
                width: 200.0,
                height: 80.0,
            },
            &mut painter,
        );
        assert_eq!(painter.0, ["Custom: 7"]);
    });
}

#[test]
fn application_components_can_receive_nested_jsx_children() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(Theme::dark()));
        let root: BoxedWidget = jsx! {
            <Panel>
                <Text>"Nested"</Text>
            </Panel>
        };
        let mut painter = TextPainter::default();
        render_frame(
            root,
            Size {
                width: 200.0,
                height: 80.0,
            },
            &mut painter,
        );
        assert_eq!(painter.0, ["Nested"]);
    });
}

#[test]
fn jsx_exposes_headless_text_and_buttons_with_layout_props() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(Theme::dark()));
        let root: BoxedWidget = Box::new(jsx! {
            <RawView style={Style::default()}>
                <RawButton style={Style::default()} background={Color::rgb(20, 20, 20)} hover_style={StateStyle::new().background(Color::rgb(30, 30, 30))} pressed_style={StateStyle::new().background(Color::rgb(10, 10, 10))} corner_radius={12.0} on_click={|| {}}>
                    <RawText color={Color::rgb(255, 200, 0)} font_size={18.0} align={TextAlign::End} style={Style::default()}>"Raw label"</RawText>
                </RawButton>
                <Text color={Color::rgb(120, 220, 255)} align={TextAlign::Start} style={Style::default()}>"Themed label"</Text>
            </RawView>
        });
        let mut painter = TextPainter::default();
        render_frame(
            root,
            Size {
                width: 200.0,
                height: 80.0,
            },
            &mut painter,
        );
        assert_eq!(painter.0, ["Raw label", "Themed label"]);
    });
}

#[test]
fn jsx_reuses_common_styles_and_inline_props_override_them() {
    let shared = CommonStyle::new()
        .width("120px")
        .background(Color::rgb(10, 20, 30))
        .corner_radius(4.0);

    let first = jsx! {
        <RawView style={shared.clone()} width={"50%"} background={Color::rgb(40, 50, 60)} />
    };
    let second = jsx! { <RawView style={shared} /> };

    let first_style = Widget::style(&first);
    let second_style = Widget::style(&second);
    assert_eq!(
        first_style.layout.size.width,
        creamui_core::layout::Dimension::Percent(0.5)
    );
    assert_eq!(
        first_style.paint.background,
        Some(Color::rgb(40, 50, 60).into())
    );
    assert_eq!(
        second_style.layout.size.width,
        creamui_core::layout::Dimension::Length(120.0)
    );
    assert_eq!(second_style.paint.corner_radius, Some(4.0));
}

#[test]
fn raw_view_accepts_a_generated_children_list() {
    let children: Vec<BoxedWidget> = vec![Box::new(creamui_widgets::raw::RawText::new(
        "Generated",
        Color::rgb(255, 255, 255),
        14.0,
    ))];
    let root: BoxedWidget = Box::new(jsx! {
        <RawView style={Style::default()} children={children} />
    });
    let mut painter = TextPainter::default();
    render_frame(
        root,
        Size {
            width: 200.0,
            height: 80.0,
        },
        &mut painter,
    );
    assert_eq!(painter.0, ["Generated"]);
}

#[test]
fn jsx_exposes_semantic_flex_layout() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(Theme::dark()));
        let root: BoxedWidget = Box::new(jsx! {
            <Flex direction={FlexDirection::Column} size={(200.0, 80.0)} gap={8.0} gap_x={12.0} align={Align::Center} justify={Justify::Center} wrap={Wrap::Wrap} padding_xy={(10.0, 6.0)}>
                <Text>"Flex child"</Text>
            </Flex>
        });
        let mut painter = TextPainter::default();
        render_frame(
            root,
            Size {
                width: 200.0,
                height: 80.0,
            },
            &mut painter,
        );
        assert_eq!(painter.0, ["Flex child"]);
    });
}

#[test]
fn jsx_exposes_semantic_block_layout() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(Theme::dark()));
        let root: BoxedWidget = Box::new(jsx! {
            <Block size={(200.0, 80.0)} padding={8.0} background={Color::rgb(20, 20, 20)}>
                <Text>"Block child"</Text>
            </Block>
        });
        let mut painter = TextPainter::default();
        render_frame(
            root,
            Size {
                width: 200.0,
                height: 80.0,
            },
            &mut painter,
        );
        assert_eq!(painter.0, ["Block child"]);
    });
}

#[test]
fn jsx_exposes_semantic_grid_layout() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(Theme::dark()));
        let root: BoxedWidget = Box::new(jsx! {
            <Grid template_columns={[Track::fr(1.0), Track::fr(1.0)]} gap={8.0} size={(200.0, 80.0)}>
                <GridItem column={1} row={1} column_span={2}>
                    <Text>"Grid child"</Text>
                </GridItem>
            </Grid>
        });
        let mut painter = TextPainter::default();
        render_frame(
            root,
            Size {
                width: 200.0,
                height: 80.0,
            },
            &mut painter,
        );
        assert_eq!(painter.0, ["Grid child"]);
    });
}

#[test]
fn jsx_exposes_configurable_portal_picker_inputs() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(Theme::dark()));
        let date = creamui_widgets::DateTimeController::new(creamui_widgets::DateTime::new(
            2026, 9, 8, 14, 30,
        ));
        let color_popup = creamui_widgets::ColorPickerController::default();
        let accent = Signal::new(Color::rgb(181, 139, 255));
        let set_accent = accent.clone();
        let root: BoxedWidget = Box::new(jsx! {
            <RawView style={Style::default()}>
                <DateInput controller={&date} popup_width={320.0} />
                <TimeInput controller={&date} minute_step={15} />
                <ColorPicker controller={&color_popup} value={accent.get()} on_change={move |color| set_accent.set(color)} />
            </RawView>
        });
        let mut painter = TextPainter::default();
        render_frame(
            root,
            Size {
                width: 500.0,
                height: 160.0,
            },
            &mut painter,
        );
        assert!(painter.0.iter().any(|text| text == "2026-09-08"));
        assert!(painter.0.iter().any(|text| text == "#B58BFF"));
    });
}

#[test]
fn jsx_exposes_raster_images() {
    let data = creamui_image::ImageData::from_rgba(1, 1, vec![255, 0, 0, 255]).unwrap();
    let root: BoxedWidget = Box::new(jsx! {
        <Image data={data} fit={creamui_image::ImageFit::Contain} corner_radius={8.0} />
    });
    let mut painter = TextPainter::default();
    render_frame(
        root,
        Size {
            width: 24.0,
            height: 24.0,
        },
        &mut painter,
    );
}
