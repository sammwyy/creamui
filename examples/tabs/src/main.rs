//! A tabs example: a horizontal bar switches which panel is shown below it,
//! built entirely with static CreamUI JSX.
//!
//! Two cards are stacked, each fully independent and clearly labeled:
//! - **Themed** — the [`Tabs`]/[`Tab`] pair, which reads every color from
//!   the app's `Theme`;
//! - **Custom** — `RawTabs`/`RawTab` built directly with hand-picked colors,
//!   showing that the raw widgets are fully stylable on their own,
//!   independent of `creamui_theme`.
//!
//! Every panel sizes itself with percentages and `flex_grow` rather than
//! fixed pixels, so the whole window reflows cleanly when resized.
//!
//! See the `sidebar` example for the vertical counterpart of this pattern.

use creamui_core::layout::{AlignItems, Dimension, FlexDirection, JustifyContent, Style};
use creamui_core::{BoxedWidget, Size, TextAlign};
use creamui_macros::{component, jsx};
use creamui_reactive::Signal;
use creamui_render::{run, WindowOptions};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::layout::{column, fixed, padding, row};
use creamui_widgets::{Card, RawTab, RawTabs, RawText, Tab, TabColors, Tabs};

struct Section {
    label: &'static str,
    title: &'static str,
    body: &'static str,
}

const SECTIONS: [Section; 4] = [
    Section {
        label: "Overview",
        title: "Overview",
        body: "A summary of what changed since your last visit.",
    },
    Section {
        label: "Activity",
        title: "Activity",
        body: "Recent events across every project you follow.",
    },
    Section {
        label: "Files",
        title: "Files",
        body: "Everything you or a teammate uploaded this week.",
    },
    Section {
        label: "Settings",
        title: "Settings",
        body: "Notification preferences, language, and appearance.",
    },
];

/// The themed tab bar: the active label picks up the accent color and a
/// capsule underline, everything else stays muted. Selection lives in the
/// caller's `Signal`, not inside the widget.
#[component]
fn TabBar(active: Signal<usize>) -> BoxedWidget {
    let theme = use_theme();
    let colors = TabColors::dark();
    let tab_style = padding(
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Auto,
                height: Dimension::Length(38.0),
            },
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            flex_grow: 1.0,
            ..Default::default()
        },
        theme.spacing_medium,
    );
    let bar_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        ..row(theme.spacing_large)
    };
    let mut tabs = Tabs::new(colors, bar_style);
    for (index, section) in SECTIONS.iter().enumerate() {
        let is_active = active.get() == index;
        let select = active.clone();
        tabs = tabs.child(Box::new(Tab::new(
            colors,
            tab_style.clone(),
            section.label,
            is_active,
            move || select.set(index),
        )));
    }
    Box::new(tabs)
}

/// A fully custom, un-themed pill-shaped tab bar built directly from
/// `RawTabs`/`RawTab`: its colors, radius, and shape have nothing to do with
/// `creamui_theme::Theme`, showing the raw widgets style however an
/// application wants.
#[component]
fn CustomTabBar(active: Signal<usize>) -> BoxedWidget {
    const BACKGROUND: Color = Color::rgb(0x1c, 0x16, 0x30);
    const REST: Color = Color::rgb(0x24, 0x1c, 0x3a);
    const ACTIVE: Color = Color::rgb(0xff, 0x5a, 0xd8);
    const TEXT_ACTIVE: Color = Color::rgb(0x1c, 0x16, 0x30);
    const TEXT_MUTED: Color = Color::rgb(0xc9, 0xb8, 0xe8);

    let pill_style = padding(
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Auto,
                height: Dimension::Length(36.0),
            },
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            flex_grow: 1.0,
            ..Default::default()
        },
        18.0,
    );
    let bar_style = padding(
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            ..row(8.0)
        },
        6.0,
    );
    let mut tabs = RawTabs::new(bar_style)
        .corner_radius(24.0)
        .background(BACKGROUND);
    for (index, section) in SECTIONS.iter().enumerate() {
        let is_active = active.get() == index;
        let select = active.clone();
        let text_color = if is_active { TEXT_ACTIVE } else { TEXT_MUTED };
        let text = RawText::new(section.label, text_color, 13.0).layout(pill_style.clone());
        // Every pill always paints a same-size background — only its color
        // changes — so selecting a tab recolors it in place instead of a
        // pill popping in where nothing was drawn before.
        let tab = RawTab::new(pill_style.clone(), is_active, move || select.set(index))
            .corner_radius(18.0)
            .background(if is_active { ACTIVE } else { REST });
        tabs = tabs.child(Box::new(tab.child(Box::new(text))));
    }
    Box::new(tabs)
}

/// The panel shown below a tab bar, holding whichever section is active.
/// Fills whatever space its card has left, so a taller window gives it more
/// room instead of leaving a gap.
#[component]
fn SectionPanel(
    background: Color,
    text_color: Color,
    muted_color: Color,
    active: Signal<usize>,
) -> BoxedWidget {
    let section = &SECTIONS[active.get()];
    let panel_style = padding(
        Style {
            flex_grow: 1.0,
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            ..column(6.0)
        },
        18.0,
    );
    Box::new(jsx! {
        <RawView style={panel_style} background={background} corner_radius={10.0}>
            <RawText color={text_color} font_size={17.0} align={TextAlign::Start} style={Style { size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Length(24.0) }, ..Default::default() }}>{section.title}</RawText>
            <RawText color={muted_color} font_size={13.0} align={TextAlign::Start} style={Style { size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Length(20.0) }, ..Default::default() }}>{section.body}</RawText>
        </RawView>
    })
}

/// A labeled card wrapping one tab bar + its content panel. `chip` is a
/// small colored square before the label, so the eye can tell the themed and
/// custom demos apart at a glance without reading the text.
#[component]
fn Showcase(chip: Color, label: String, children: Vec<BoxedWidget>) -> BoxedWidget {
    let theme = use_theme();
    // The outer column just stacks the caption above the card; only it
    // carries `flex_grow`, so `column()`'s split of the window is unaffected
    // by the card's own padding.
    let outer_style = Style {
        flex_grow: 1.0,
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        ..column(theme.spacing_small)
    };
    let header_style = Style {
        align_items: Some(AlignItems::Center),
        flex_shrink: 0.0,
        ..row(theme.spacing_small)
    };
    let chip_style = Style {
        size: fixed(8.0, 8.0),
        flex_shrink: 0.0,
        ..Default::default()
    };
    let label_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Length(16.0),
        },
        ..Default::default()
    };
    let card_style = padding(
        Style {
            flex_grow: 1.0,
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            flex_direction: FlexDirection::Column,
            ..column(theme.spacing_medium)
        },
        theme.spacing_large,
    );
    let header: BoxedWidget = Box::new(jsx! {
        <RawView style={header_style}>
            <RawView style={chip_style} background={chip} corner_radius={2.0} />
            <RawText color={theme.text_disabled} font_size={12.0} align={TextAlign::Start} style={label_style}>{label}</RawText>
        </RawView>
    });
    let card: BoxedWidget = Box::new(Card::new(card_style).with_children(children));
    Box::new(jsx! {
        <RawView style={outer_style}>
            {header}
            {card}
        </RawView>
    })
}

fn main() {
    let themed_active = Signal::new(0usize);
    let custom_active = Signal::new(1usize);
    run(
        WindowOptions {
            title: "CreamUI — Tabs".into(),
            width: 560,
            height: 640,
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
                    flex_direction: FlexDirection::Column,
                    ..column(theme.spacing_large)
                },
                theme.spacing_large,
            );
            Box::new(jsx! {
                <RawView style={root_style} background={theme.surface}>
                    <Showcase chip={theme.accent} label={"THEMED — Tabs / Tab".to_owned()} children={vec![
                        TabBar(TabBarProps { active: themed_active.clone() }),
                        SectionPanel(SectionPanelProps { background: theme.surface_elevated, text_color: theme.text_primary, muted_color: theme.text_secondary, active: themed_active.clone() }),
                    ]} />
                    <Showcase chip={Color::rgb(0xff, 0x5a, 0xd8)} label={"CUSTOM — RawTabs / RawTab".to_owned()} children={vec![
                        CustomTabBar(CustomTabBarProps { active: custom_active.clone() }),
                        SectionPanel(SectionPanelProps { background: Color::rgb(0x14, 0x10, 0x24), text_color: Color::rgb(0xf1, 0xe6, 0xff), muted_color: Color::rgb(0xc9, 0xb8, 0xe8), active: custom_active.clone() }),
                    ]} />
                </RawView>
            })
        },
    );
}
use creamui_core::Styled as _;
