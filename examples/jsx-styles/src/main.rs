use creamui::core::{
    layout::{AlignItems, Display, FlexDirection, JustifyContent},
    TextAlign,
};
use creamui::{jsx, run, ColorToken, Signal, Size, Style, Theme, WindowOptions};

fn action_style() -> Style {
    Style::new()
        .width(220.0)
        .height(46.0)
        .display(Display::Flex)
        .align_items(AlignItems::Center)
        .justify_content(JustifyContent::Center)
        .background(ColorToken::SurfaceElevated)
        .border(ColorToken::Border, 1.0)
        .corner_radius(8.0)
        .color(ColorToken::TextPrimary)
        .padding(12.0)
}

fn main() {
    let clicks = Signal::new(0_u32);
    run(
        WindowOptions {
            title: "CreamUI — JSX styles".into(),
            width: 520,
            height: 340,
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
                .gap(12.0);

            Box::new(jsx! {
                <RawView style={screen}>
                    <RawText font_size={22.0} color={ColorToken::TextPrimary} align={TextAlign::Center}>
                        "Shared JSX styles"
                    </RawText>
                    <RawButton style={shared.clone()} on_click={move || increment.update(|value| *value += 1)}>
                        <RawText color={ColorToken::TextPrimary} font_size={14.0} align={TextAlign::Center}>
                            "Increment"
                        </RawText>
                    </RawButton>
                    <RawButton style={shared} background={ColorToken::Accent} on_click={move || reset.set(0)}>
                        <RawText color={ColorToken::SelectionText} font_size={14.0} align={TextAlign::Center}>
                            "Reset with inline override"
                        </RawText>
                    </RawButton>
                    <RawText width={220.0} height={32.0} background={ColorToken::SurfaceHover} corner_radius={6.0} align={TextAlign::Center}>
                        {format!("Clicks: {}", clicks.get())}
                    </RawText>
                </RawView>
            })
        },
    );
}
