//! A sidebar example: a vertical rail switches which panel is shown next to
//! it, built entirely with static CreamUI JSX.
//!
//! Two cards sit side by side, each fully independent and clearly labeled:
//! - **Themed** — the [`Sidebar`]/[`SidebarItem`] pair, which reads every
//!   color from the app's `Theme`;
//! - **Custom** — `RawSidebar`/`RawTab` built directly with hand-picked
//!   colors, showing that the raw widgets are fully stylable on their own,
//!   independent of `creamui_theme`.
//!
//! Every panel sizes itself with percentages and `flex_grow` rather than
//! fixed pixels, so the whole window reflows cleanly when resized.
//!
//! See the `tabs` example for the horizontal counterpart of this pattern.

use creamui_core::layout::{AlignItems, Dimension, Style};
use creamui_core::{BoxedWidget, Size, TextAlign};
use creamui_macros::{component, jsx};
use creamui_reactive::Signal;
use creamui_render::{run, WindowOptions};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::layout::{column, fixed, padding, row};
use creamui_widgets::raw::TabIndicatorSide;
use creamui_widgets::{Card, RawSidebar, RawTab, RawText, Sidebar, SidebarItem, TabColors};

struct Section {
    label: &'static str,
    title: &'static str,
    body: &'static str,
}

const SECTIONS: [Section; 4] = [
    Section {
        label: "Dashboard",
        title: "Dashboard",
        body: "What changed since your last visit.",
    },
    Section {
        label: "Profile",
        title: "Profile",
        body: "Your name, avatar, and visibility.",
    },
    Section {
        label: "Settings",
        title: "Settings",
        body: "Notifications, language, appearance.",
    },
    Section {
        label: "About",
        title: "About",
        body: "Version, license, and issue tracker.",
    },
];

/// The themed sidebar rail: a soft rounded highlight and accent bar mark the
/// active section, muted text everywhere else. Selection lives in the
/// caller's `Signal`, not inside the widget.
#[component]
fn SidebarNav(active: Signal<usize>) -> BoxedWidget {
    let theme = use_theme();
    let colors = TabColors::sidebar();
    let item_style = padding(
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(38.0),
            },
            align_items: Some(AlignItems::Center),
            ..Default::default()
        },
        theme.spacing_medium,
    );
    let sidebar_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(160.0),
            height: Dimension::Auto,
        },
        flex_shrink: 0.0,
        ..column(2.0)
    };
    let mut sidebar = Sidebar::new(colors, sidebar_style);
    for (index, section) in SECTIONS.iter().enumerate() {
        let is_active = active.get() == index;
        let select = active.clone();
        sidebar = sidebar.child(Box::new(SidebarItem::new(
            colors,
            item_style.clone(),
            section.label,
            is_active,
            move || select.set(index),
        )));
    }
    // Match the content card's inset: the rail keeps the canvas colour and
    // its rounded item backgrounds never touch the card edge.
    let outer_style = padding(
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Length(160.0 + theme.spacing_medium * 2.0),
                height: Dimension::Percent(1.0),
            },
            flex_shrink: 0.0,
            ..Default::default()
        },
        theme.spacing_medium,
    );
    Box::new(jsx! {
        <RawView style={outer_style} background={theme.surface}>
            {Box::new(sidebar) as BoxedWidget}
        </RawView>
    })
}

/// A fully custom, un-themed sidebar built directly from `RawSidebar`/
/// `RawTab`: its colors and shape have nothing to do with
/// `creamui_theme::Theme`, showing the raw widgets style however an
/// application wants.
#[component]
fn CustomSidebar(active: Signal<usize>) -> BoxedWidget {
    const ACTIVE: Color = Color::rgb(0x2e, 0xe6, 0x7a);
    const TEXT_ACTIVE: Color = Color::rgb(0x2e, 0xe6, 0x7a);
    const TEXT_MUTED: Color = Color::rgb(0x74, 0xa8, 0x86);

    let item_style = padding(
        Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(34.0),
            },
            align_items: Some(AlignItems::Center),
            ..Default::default()
        },
        12.0,
    );
    let sidebar_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(160.0),
            height: Dimension::Auto,
        },
        flex_shrink: 0.0,
        ..column(4.0)
    };
    let mut sidebar = RawSidebar::new(sidebar_style);
    for (index, section) in SECTIONS.iter().enumerate() {
        let is_active = active.get() == index;
        let select = active.clone();
        let text_color = if is_active { TEXT_ACTIVE } else { TEXT_MUTED };
        let label = format!("> {}", section.label);
        let text = RawText::new(label, text_color, 13.0)
            .text_align(TextAlign::Start)
            .layout(item_style.clone());
        // No background fill: a full-row block popping in and out on every
        // click reads as the whole row changing size, not just selection.
        // Only the text color and a thin indicator bar change.
        let mut item = RawTab::new(item_style.clone(), is_active, move || select.set(index));
        if is_active {
            item = item.indicator(TabIndicatorSide::Left, ACTIVE, 3.0);
        }
        sidebar = sidebar.child(Box::new(item.child(Box::new(text))));
    }
    Box::new(sidebar)
}

/// The panel shown next to a sidebar, holding whichever section is active.
/// Fills whatever space its card has left, so a taller or wider window gives
/// it more room instead of leaving a gap.
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
                width: Dimension::Auto,
                height: Dimension::Percent(1.0),
            },
            ..column(6.0)
        },
        18.0,
    );
    let title_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Length(22.0),
        },
        ..Default::default()
    };
    let body_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Length(40.0),
        },
        ..Default::default()
    };
    Box::new(jsx! {
        <RawView style={panel_style} background={background} corner_radius={10.0}>
            <RawText color={text_color} font_size={16.0} align={TextAlign::Start} style={title_style}>{section.title}</RawText>
            <RawText color={muted_color} font_size={13.0} align={TextAlign::Start} style={body_style}>{section.body}</RawText>
        </RawView>
    })
}

/// A labeled card wrapping one sidebar + its content panel. `chip` is a
/// small colored square before the label, so the eye can tell the themed and
/// custom demos apart at a glance without reading the text.
#[component]
fn Showcase(chip: Color, label: String, children: Vec<BoxedWidget>) -> BoxedWidget {
    let theme = use_theme();
    // The outer column just stacks the caption above the card; only it
    // carries `flex_grow`, so `row()`'s split of the window is unaffected by
    // the card's own padding.
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
                width: Dimension::Auto,
                height: Dimension::Percent(1.0),
            },
            // stretch the rail to the panel's full height, not `row()`'s default center
            align_items: Some(AlignItems::Stretch),
            ..row(theme.spacing_medium)
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
            title: "CreamUI — Sidebar".into(),
            width: 1040,
            height: 520,
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
                    // stretch cards to full height, not `row()`'s default center
                    align_items: Some(AlignItems::Stretch),
                    ..row(theme.spacing_large)
                },
                theme.spacing_large,
            );
            Box::new(jsx! {
                <RawView style={root_style} background={theme.surface}>
                    <Showcase chip={theme.accent} label={"THEMED — Sidebar / SidebarItem".to_owned()} children={vec![
                        SidebarNav(SidebarNavProps { active: themed_active.clone() }),
                        SectionPanel(SectionPanelProps { background: theme.surface_elevated, text_color: theme.text_primary, muted_color: theme.text_secondary, active: themed_active.clone() }),
                    ]} />
                    <Showcase chip={Color::rgb(0x2e, 0xe6, 0x7a)} label={"CUSTOM — RawSidebar / RawTab".to_owned()} children={vec![
                        CustomSidebar(CustomSidebarProps { active: custom_active.clone() }),
                        SectionPanel(SectionPanelProps { background: Color::rgb(0x0c, 0x1a, 0x11), text_color: Color::rgb(0xd6, 0xf5, 0xe1), muted_color: Color::rgb(0x74, 0xa8, 0x86), active: custom_active.clone() }),
                    ]} />
                </RawView>
            })
        },
    );
}
use creamui_core::Styled as _;
