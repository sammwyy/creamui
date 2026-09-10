use creamui_core::layout::{FlexDirection, Style};
use creamui_core::{render_frame, BoxedWidget, Painter, Rect, Size, TextAlign};
use creamui_macros::{abi_jsx, component, jsx};
use creamui_reactive::Signal;
use creamui_theme::{Color, Theme};
use creamui_widgets::layout::{Align, Justify, Track, Wrap};

#[derive(Default)]
struct TextPainter(Vec<String>);

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
                <RawButton style={Style::default()} background={Color::rgb(20, 20, 20)} hover_background={Color::rgb(30, 30, 30)} pressed_background={Color::rgb(10, 10, 10)} corner_radius={12.0} on_click={|| {}}>
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
