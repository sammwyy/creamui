//! Reusable and inline common styles on arbitrary widgets. Run with
//! `cargo run -p universal-styles`.

use creamui_core::layout::{AlignItems, Display, FlexDirection, JustifyContent, Style as Layout};
use creamui_core::{BoxedWidget, ColorToken, Size, StateStyle, Style, Styled, TextAlign};
use creamui_reactive::Signal;
use creamui_render::{run, WindowOptions};
use creamui_theme::Theme;
use creamui_widgets::{RawButton, RawText, RawView};

fn action_style() -> Style {
    Style::new()
        .width(220.0)
        .height(46.0)
        .background(ColorToken::SurfaceElevated)
        .border(ColorToken::Border, 1.0)
        .corner_radius(8.0)
        .hover(StateStyle::new().background(ColorToken::AccentHover))
        .pressed(StateStyle::new().background(ColorToken::AccentPressed))
        .focus(StateStyle::new().outline(ColorToken::Accent, 2.0))
}

fn action(label: impl Into<String>, style: &Style, on_click: impl Fn() + 'static) -> BoxedWidget {
    let label = RawText::new(label, Theme::dark().text_primary, 14.0)
        .align(TextAlign::Center)
        .width(220.0);
    Box::new(
        RawButton::new(Layout::default(), on_click)
            .child(Box::new(label))
            .with_style(style.clone()),
    )
}

fn main() {
    let clicks = Signal::new(0_u32);
    run(
        WindowOptions {
            title: "CreamUI — Universal styles".into(),
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

            let title = RawText::new("One Style, many widgets", Theme::dark().text_primary, 22.0)
                .bold(true)
                .styled()
                .color(ColorToken::TextPrimary);
            let count = RawText::new(
                format!("Clicks: {}", clicks.get()),
                Theme::dark().text_secondary,
                14.0,
            )
            // Inline common properties are available on every Widget.
            .width(220.0)
            .height(32.0)
            .background(ColorToken::SurfaceHover)
            .corner_radius(6.0);

            Box::new(
                RawView::new(Layout {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_items: Some(AlignItems::Center),
                    justify_content: Some(JustifyContent::Center),
                    gap: creamui_core::layout::Size {
                        width: creamui_core::layout::LengthPercentage::Length(12.0),
                        height: creamui_core::layout::LengthPercentage::Length(12.0),
                    },
                    ..Default::default()
                })
                .with_children(vec![
                    Box::new(title),
                    action("Increment", &shared, move || increment.update(|n| *n += 1)),
                    action("Reset", &shared, move || reset.set(0)),
                    Box::new(count),
                ])
                .width(viewport.width)
                .height(viewport.height),
            )
        },
    );
}
