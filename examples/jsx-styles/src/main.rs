use creamui::core::{
    layout::{AlignItems, BoxSizing, Display, FlexDirection, JustifyContent},
    TextAlign,
};
use creamui::{
    jsx, run, BoxShadow, Color, ColorToken, Signal, Size, StateStyle, Style, Theme, WindowOptions,
};

fn action_style() -> Style {
    Style::new()
        .width(180.0)
        .height(46.0)
        .display(Display::Flex)
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .background(ColorToken::SurfaceElevated)
        .border(ColorToken::Border, 1.0)
        .corner_radius(8.0)
        .color(ColorToken::TextPrimary)
        .box_sizing(BoxSizing::BorderBox)
        .padding("8px 12px")
        .hover(StateStyle::new().border(ColorToken::Accent, 2.0))
}

fn main() {
    let clicks = Signal::new(0_u32);
    run(
        WindowOptions {
            title: "CreamUI — JSX styles".into(),
            width: 800,
            height: 620,
            theme: Theme::dark(),
            ..Default::default()
        },
        Theme::dark().surface,
        |_| {},
        move |viewport: Size| {
            let shared = action_style();
            let increment = clicks.clone();
            let reset = clicks.clone();
            let screen = Style::new()
                .width(viewport.width)
                .height(viewport.height)
                .display(Display::Flex)
                .flex_direction(FlexDirection::Column)
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::Center)
                .gap(14.0)
                .padding("20px 24px");

            Box::new(jsx! {
                <RawView style={screen}>
                    <Block width={720.0} height={88.0} padding={"16px 24px"} corner_radius={14.0}
                        border={(ColorToken::BorderStrong, 2.0)}
                        box_shadow={BoxShadow::new(0.0, 10.0, 20.0, 0.0, Color::rgba(0, 0, 0, 96))}
                        background={"radial-gradient(circle at 20% 20%, #587cff, #121830)"}>
                        <RawText height={28.0} font_size={22.0} bold={true} color={Color::rgb(255, 255, 255)}>
                            "Native JSX styling"
                        </RawText>
                        <RawText height={22.0} margin={"0 0 0 4px"} font_size={13.0} color={Color::rgb(220, 230, 255)}>
                            "Radial background, inner border, shadow and padded text"
                        </RawText>
                    </Block>
                    <Flex direction={FlexDirection::Row} gap={16.0} padding={"0 12px"}>
                        <Block width={320.0} height={126.0} padding={"16px 20px"}
                            box_sizing={BoxSizing::BorderBox} border={(ColorToken::BorderStrong, 4.0)}
                            corner_radius={12.0} background={ColorToken::SurfaceElevated}>
                            <RawText height={28.0} color={ColorToken::TextPrimary} font_size={17.0}>
                                "border-box: 320 × 126"
                            </RawText>
                            <RawText color={ColorToken::TextSecondary} font_size={13.0}>
                                "Padding and border fit inside the size"
                            </RawText>
                        </Block>
                        <Block width={276.0} height={86.0} padding={"16px 20px"}
                            box_sizing={BoxSizing::ContentBox} border={(ColorToken::Accent, 2.0)}
                            corner_radius={12.0} background={"linear-gradient(135deg, #243c63, #172439)"}>
                            <RawText height={28.0} color={ColorToken::TextPrimary} font_size={17.0}>
                                "content-box: 276 × 86"
                            </RawText>
                            <RawText color={ColorToken::TextSecondary} font_size={13.0}>
                                "Padding and border enlarge this box"
                            </RawText>
                        </Block>
                    </Flex>
                    <Grid columns={3} gap={12.0} width={720.0} padding={"12px 16px"}
                        border={(ColorToken::Border, 1.0)} corner_radius={12.0}
                        background={ColorToken::SurfaceElevated}>
                        <RawView height={74.0} corner_radius={8.0}
                            background={"linear-gradient(90deg, #587cff, #a07aff)"}>
                            <RawText padding={"12px 14px"} color={Color::rgb(255, 255, 255)} font_size={14.0}>
                                "Linear gradient"
                            </RawText>
                        </RawView>
                        <RawView height={74.0} corner_radius={8.0}
                            background={"radial-gradient(circle at 25% 25%, #7fb5ff, #243458)"}>
                            <RawText padding={"12px 14px"} color={Color::rgb(255, 255, 255)} font_size={14.0}>
                                "Radial gradient"
                            </RawText>
                        </RawView>
                        <RawView height={74.0} corner_radius={8.0} border={(ColorToken::Accent, 3.0)}
                            box_shadow={BoxShadow::new(0.0, 5.0, 10.0, 0.0, Color::rgba(0, 0, 0, 90))}
                            background={ColorToken::SurfaceHover}>
                            <RawText padding={"12px 14px"} color={ColorToken::TextPrimary} font_size={14.0}>
                                "Border and shadow"
                            </RawText>
                        </RawView>
                    </Grid>
                    <Flex direction={FlexDirection::Row} gap={12.0} padding={"8px 16px"}>
                        <RawButton style={shared.clone()} on_click={move || increment.update(|value| *value += 1)}>
                            <RawText color={ColorToken::TextPrimary} font_size={14.0} align={TextAlign::Center}>
                                "Increment"
                            </RawText>
                        </RawButton>
                        <RawButton style={shared} background={ColorToken::Accent} on_click={move || reset.set(0)}>
                            <RawText color={ColorToken::SelectionText} font_size={14.0} align={TextAlign::Center}>
                                "Reset"
                            </RawText>
                        </RawButton>
                    </Flex>
                    <RawText width={720.0} height={44.0} padding={"10px 16px"}
                        background={ColorToken::SurfaceHover} border={(ColorToken::Border, 1.0)}
                        corner_radius={8.0} color={ColorToken::TextPrimary} font_size={14.0}>
                        {format!("Clicks: {} — text stays inside its padded content box", clicks.get())}
                    </RawText>
                </RawView>
            })
        },
    );
}
