//! A compact gallery for CreamUI's semantic flex layout API.

use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_render::{run, WindowOptions};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::layout::{Align, Justify, StyleExt, Wrap};
use creamui_widgets::RawText;

fn card(title: &'static str, description: &'static str, accent: Color) -> BoxedWidget {
    let theme = use_theme();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(220.0, 142.0)} gap={8.0} padding={16.0} background={theme.surface_elevated} corner_radius={theme.card_radius}>
            {Box::new(RawText::new(title, accent, 17.0).bold(true)) as BoxedWidget}
            <RawText color={theme.text_secondary} font_size={13.0} style={creamui_core::layout::Style::default().grow(1.0)}>{description}</RawText>
        </Flex>
    })
}

fn main() {
    run(
        WindowOptions {
            title: "CreamUI — Flex layout".into(),
            width: 760,
            height: 500,
            theme: Theme::dark(),
            ..Default::default()
        },
        Theme::dark().surface,
        |_| {},
        move |viewport: Size| -> BoxedWidget {
            let theme = use_theme();

            let header = Box::new(jsx! {
                <Flex align={Align::Center} justify={Justify::Between}>
                    <Text font_size={24.0}>"Flex, without the style boilerplate"</Text>
                    <RawText color={theme.accent} font_size={13.0} style={creamui_core::layout::Style::default().padding_all(8.0)}>"display: flex"</RawText>
                </Flex>
            }) as BoxedWidget;

            let cards = Box::new(jsx! {
                <Flex grow={1.0} gap={14.0} wrap={Wrap::Wrap} align_content={Justify::Start}>
                    {card(
                        "Grid",
                        "Cards wrap onto another line when space runs out.",
                        theme.accent,
                    )}
                    {card(
                        "Gap",
                        "Horizontal and vertical gaps are independent.",
                        Color::rgb(0x8b, 0x5c, 0xf6),
                    )}
                    {card(
                        "Alignment",
                        "Items stay centered on the cross axis.",
                        Color::rgb(0x22, 0xc5, 0x5e),
                    )}
                    {card(
                        "Grow",
                        "The gallery takes the remaining height.",
                        Color::rgb(0xf5, 0x9e, 0x0b),
                    )}
                </Flex>
            }) as BoxedWidget;

            let footer = Box::new(jsx! {
                <Flex justify={Justify::End} align={Align::Center}>
                    <RawText color={theme.text_disabled} font_size={12.0}>"Flex · Wrap · Gap · StyleExt"</RawText>
                </Flex>
            }) as BoxedWidget;

            Box::new(jsx! {
                <Flex direction={FlexDirection::Column} size={(viewport.width, viewport.height)} gap={20.0} padding={24.0} background={theme.surface}>
                    {header}
                    {cards}
                    {footer}
                </Flex>
            })
        },
    );
}
use creamui_core::Styled as _;
