//! The sidebar category rail.

use crate::prelude::*;

/// The showcase category rail.
#[component]
pub fn Nav(
    active: Signal<ShowcasePage>,
    content_scroll: ScrollController,
    nav_scroll: ScrollController,
    sidebar_open: Signal<bool>,
    width: f32,
    insets: SafeArea,
) -> BoxedWidget {
    let theme = use_theme();
    // Wider than before, and padded almost only on the left: the card gap
    // to its right already separates it from the content panel, so giving
    // it a matching right pad on top of that would just waste width.
    let nav_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(width),
            height: Dimension::Percent(1.),
        },
        flex_shrink: 0.,
        padding: creamui_core::layout::Rect {
            left: creamui_core::layout::LengthPercentage::Length(16. + insets.left),
            right: creamui_core::layout::LengthPercentage::Length(6. + insets.right),
            top: creamui_core::layout::LengthPercentage::Length(16. + insets.top),
            bottom: creamui_core::layout::LengthPercentage::Length(16. + insets.bottom),
        },
        ..column(5.)
    };
    let mut items: Vec<BoxedWidget> = Vec::new();
    let mut group = None;
    for entry in &NAV_PAGES {
        if group != Some(entry.group) {
            group = Some(entry.group);
            let heading = match entry.group {
                NavGroup::Features => "FEATURES",
                NavGroup::Components => "COMPONENTS",
            };
            items.push(Box::new(jsx! {
                <RawView style={padding(column(0.), 8.)}>
                    <RawText color={theme.text_secondary} font_size={10.0} align={TextAlign::Start}>{heading.to_owned()}</RawText>
                </RawView>
            }));
        }
        let select = active.clone();
        let reset_scroll = content_scroll.clone();
        let close_sidebar = sidebar_open.clone();
        items.push(jsx! {
            <NavigationItem
                symbol={entry.symbol}
                label={entry.label.to_owned()}
                active={active.get() == entry.page}
                on_click={Box::new(move || {
                    select.set(entry.page);
                    reset_scroll.set(0.0);
                    close_sidebar.set(false);
                }) as Box<dyn Fn()>}
            />
        });
    }
    // Grows to fill whatever space is left between the logo and the footer
    // below, same as the plain spacer `RawView` this replaces — the only
    // difference is that once the item list is taller than that space, it
    // scrolls (draggable thumb included) instead of pushing the footer off
    // the bottom of the window.
    let items_style = Style {
        flex_grow: 1.,
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Percent(1.0),
        },
        ..Default::default()
    };
    Box::new(jsx! {
        <RawView style={nav_style}>
            <RawView style={padding(row(8.), 8.)}>
                <Icon symbol={Symbol::Appearance} color={theme.accent} size={24.0} />
                <BoldText text={"CreamUI".to_owned()} color={theme.text_primary} font_size={19.0} align={TextAlign::Start} />
            </RawView>
            <RawScrollView
                style={items_style}
                controller={nav_scroll}
                content_gap={Some(5.0)}
                scrollbar_gap={Some(6.0)}
                background={None}
                corner_radius={None}
                scrollbar_width={None}
                scrollbar_color={None}
                scrollbar_hover_color={None}
                children={items}
            />
            <RawView style={padding(column(5.), 8.)}>
                <RawText color={theme.text_secondary} font_size={11.0} align={TextAlign::Start}>"Component library"</RawText>
                <RawText color={theme.text_disabled} font_size={11.0} align={TextAlign::Start}>"CreamUI · 0.1"</RawText>
            </RawView>
        </RawView>
    })
}
