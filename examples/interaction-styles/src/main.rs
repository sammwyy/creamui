//! Hover and pressed-state styling for a raw button. Run with
//! `cargo run -p interaction-styles`.

use creamui_core::layout::Style;
use creamui_core::{BoxedWidget, Size, TextAlign};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::{run, WindowOptions};
use creamui_theme::{use_theme, Theme};
use creamui_widgets::layout::{fixed, Align, Justify};
use creamui_widgets::{ButtonVisualStyle, RawButtonStyle};

fn main() {
    let clicks = Signal::new(0_u32);
    run(
        WindowOptions {
            title: "CreamUI — Interaction styles".into(),
            width: 560,
            height: 360,
            theme: Theme::dark(),
            ..Default::default()
        },
        Theme::dark().surface,
        |_| {},
        move |viewport: Size| -> BoxedWidget {
            let theme = use_theme();
            let count = clicks.get();
            let increment = clicks.clone();
            let button_style = Style {
                size: fixed(260., 54.),
                ..Default::default()
            };
            // This value can be stored, cloned, and shared by every button
            // that should have the same visual language.
            let action_style = RawButtonStyle::new(button_style.clone())
                .background(theme.surface_elevated)
                .border(theme.border, 1.)
                .corner_radius(8.)
                .hover(
                    ButtonVisualStyle::new()
                        .background(theme.accent_hover)
                        .border(theme.accent, 2.)
                        .corner_radius(14.),
                )
                .pressed(
                    ButtonVisualStyle::new()
                        .background(theme.accent_pressed)
                        .border(theme.selection_text, 2.)
                        .corner_radius(4.),
                );
            let custom_button: BoxedWidget = Box::new(jsx! {
                <RawButton style={action_style} on_click={move || increment.update(|n| *n += 1)}>
                    <RawText color={theme.selection_text} font_size={16.0} align={TextAlign::Center}>"Hover and hold me"</RawText>
                </RawButton>
            });

            Box::new(jsx! {
                <Flex direction={creamui_core::layout::FlexDirection::Column} size={(viewport.width, viewport.height)} gap={16.0} justify={Justify::Center} align={Align::Center} background={theme.surface}>
                    <Text font_size={24.0}>"Interaction styles"</Text>
                    <Text color={theme.text_secondary}>"The first button changes background, border, and radius."</Text>
                    {custom_button}
                    <RawButton style={button_style} background={theme.surface_elevated} hover_background={theme.surface_hover} pressed_background={theme.border_strong} corner_radius={8.0} on_click={|| {}}>
                        <RawText color={theme.text_primary} font_size={14.0} align={TextAlign::Center}>"JSX convenience props"</RawText>
                    </RawButton>
                    <Text color={theme.text_secondary}>{format!("Clicks: {count}")}</Text>
                </Flex>
            })
        },
    );
}
