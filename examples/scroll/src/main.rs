//! A scroll example: three lists demonstrate `RawScrollView`'s draggable
//! scrollbar (`RawScrollbar`), built entirely with static CreamUI JSX.
//!
//! Three cards sit side by side, each independently scrollable:
//! - **Themed** — the [`ScrollView`] wrapper, whose scrollbar picks up its
//!   colors from the app's `Theme`;
//! - **Custom** — `RawScrollView` with hand-picked scrollbar colors, showing
//!   the raw widget is fully stylable on its own;
//! - **Wheel only** — the same `RawScrollView`, but with
//!   `.scrollbar(false)`: the mouse wheel still scrolls it, just without the
//!   draggable thumb.
//!
//! Drag any visible thumb, or turn the mouse wheel over any list — both
//! move the same shared `ScrollController`, so they always agree on where
//! the content is.
//!
//! See the `tabs`/`sidebar` examples for the same card layout applied to
//! other controls.

use creamui_core::layout::{AlignItems, Dimension, Style};
use creamui_core::{BoxedWidget, Size, TextAlign};
use creamui_macros::{component, jsx};
use creamui_render::{run, WindowOptions};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::layout::{column, padding, row};
use creamui_widgets::{Card, RawScrollView, ScrollController, ScrollView};

/// A row of numbered list items, tall enough in aggregate to overflow every
/// list in this example.
const ROW_COUNT: usize = 32;

fn row_style(theme: &Theme) -> Style {
    padding(
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(32.0),
            },
            align_items: Some(AlignItems::Center),
            ..Default::default()
        },
        theme.spacing_medium,
    )
}

/// Builds `ROW_COUNT` alternating-background rows, each just a numbered
/// label — enough content for a 260px-tall list to need scrolling.
fn rows(theme: &Theme, text_color: Color) -> Vec<BoxedWidget> {
    let text_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Percent(1.0),
        },
        ..Default::default()
    };
    (0..ROW_COUNT)
        .map(|i| {
            let background = if i % 2 == 0 {
                theme.surface_elevated
            } else {
                theme.surface
            };
            Box::new(jsx! {
                <RawView style={row_style(theme)} background={background}>
                    <RawText color={text_color} font_size={13.0} align={TextAlign::Start} style={text_style.clone()}>
                        {format!("Item {:02}", i + 1)}
                    </RawText>
                </RawView>
            }) as BoxedWidget
        })
        .collect()
}

fn list_style() -> Style {
    Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(220.0),
            height: Dimension::Length(280.0),
        },
        flex_shrink: 0.0,
        ..Default::default()
    }
}

/// A labeled card wrapping one scrollable list. `chip` is a small colored
/// square before the label, so the eye can tell the three demos apart at a
/// glance without reading the text.
#[component]
fn Card(chip: Color, label: String, list: BoxedWidget) -> BoxedWidget {
    let theme = use_theme();
    let outer_style = Style {
        flex_grow: 1.0,
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Percent(1.0),
        },
        ..column(theme.spacing_small)
    };
    let header_style = Style {
        align_items: Some(AlignItems::Center),
        flex_shrink: 0.0,
        ..row(theme.spacing_small)
    };
    let chip_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(8.0),
            height: Dimension::Length(8.0),
        },
        flex_shrink: 0.0,
        ..Default::default()
    };
    let card_style = padding(
        Style {
            flex_grow: 1.0,
            size: creamui_core::layout::Size {
                width: Dimension::Auto,
                height: Dimension::Percent(1.0),
            },
            align_items: Some(AlignItems::Center),
            justify_content: Some(creamui_core::layout::JustifyContent::Center),
            ..column(theme.spacing_medium)
        },
        theme.spacing_large,
    );
    let header: BoxedWidget = Box::new(jsx! {
        <RawView style={header_style}>
            <RawView style={chip_style} background={chip} corner_radius={2.0} />
            <Text align={TextAlign::Start} color={theme.text_disabled} style={Style { size: creamui_core::layout::Size { width: Dimension::Auto, height: Dimension::Length(16.0) }, ..Default::default() }}>{label}</Text>
        </RawView>
    });
    let card: BoxedWidget = Box::new(Card::new(card_style).child(list));
    Box::new(jsx! {
        <RawView style={outer_style}>
            {header}
            {card}
        </RawView>
    })
}

fn main() {
    let themed_scroll = ScrollController::default();
    let custom_scroll = ScrollController::default();
    let wheel_only_scroll = ScrollController::default();

    run(
        WindowOptions {
            title: "CreamUI — Scroll".into(),
            width: 900,
            height: 460,
            theme: Theme::dark(),
            ..Default::default()
        },
        Theme::dark().surface,
        |_| {},
        move |viewport: Size| -> BoxedWidget {
            let theme = use_theme();
            let root_style = padding(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(viewport.width),
                        height: Dimension::Length(viewport.height),
                    },
                    align_items: Some(AlignItems::Stretch),
                    ..row(theme.spacing_large)
                },
                theme.spacing_large,
            );

            let themed_list = Box::new(
                ScrollView::controlled(list_style(), themed_scroll.clone())
                    .with_children(rows(&theme, theme.text_primary)),
            ) as BoxedWidget;

            const NEON: Color = Color::rgb(0x5c, 0xe1, 0xff);
            let custom_list = Box::new(
                RawScrollView::controlled(list_style(), custom_scroll.clone())
                    .background(Color::rgb(0x0c, 0x14, 0x1a))
                    .corner_radius(theme.radius_medium)
                    .scrollbar_width(7.0)
                    .scrollbar_color(Color::rgba(NEON.r, NEON.g, NEON.b, 150))
                    .scrollbar_hover_color(Color::rgba(NEON.r, NEON.g, NEON.b, 220))
                    .with_children(rows(&theme, Color::rgb(0xbf, 0xef, 0xff))),
            ) as BoxedWidget;

            let wheel_only_list = Box::new(
                RawScrollView::controlled(list_style(), wheel_only_scroll.clone())
                    .background(theme.surface_elevated)
                    .corner_radius(theme.radius_medium)
                    .scrollbar(false)
                    .with_children(rows(&theme, theme.text_secondary)),
            ) as BoxedWidget;

            Box::new(jsx! {
                <RawView style={root_style} background={theme.surface}>
                    <Card chip={theme.accent} label={"THEMED — ScrollView".to_owned()} list={themed_list} />
                    <Card chip={NEON} label={"CUSTOM — RawScrollView".to_owned()} list={custom_list} />
                    <Card chip={theme.text_disabled} label={"WHEEL ONLY — scrollbar(false)".to_owned()} list={wheel_only_list} />
                </RawView>
            })
        },
    );
}
use creamui_core::Styled as _;
