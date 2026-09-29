//! The sidebar category rail.

use crate::prelude::*;

/// The showcase category rail.
#[component]
pub fn Nav(
    active: Signal<usize>,
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
    let symbols = [
        Symbol::Appearance,
        Symbol::Display,
        Symbol::Keyboard,
        Symbol::Controls,
        Symbol::Display,
        Symbol::Controls,
        Symbol::Sliders,
        Symbol::Check,
        Symbol::Controls,
        Symbol::Display,
        Symbol::Folder,
        Symbol::Grid,
        Symbol::Grid,
        Symbol::Folder,
        Symbol::Grid,
        Symbol::Grid,
        Symbol::Controls,
    ];
    let mut items: Vec<BoxedWidget> = Vec::new();
    for (i, label) in NAV_LABELS.iter().enumerate() {
        if i == 0 || i == 2 || i == 8 || i == 10 || i == 13 || i == 15 {
            let heading = if i == 0 {
                "SHOWCASE"
            } else if i == 2 {
                "CONTROLS"
            } else if i == 8 {
                "SELECTION"
            } else if i == 13 {
                "DATA VIEW"
            } else if i == 15 {
                "LAYOUT"
            } else {
                "NAVIGATION"
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
                symbol={symbols[i]}
                label={(*label).to_owned()}
                active={active.get() == i}
                on_click={Box::new(move || {
                    select.set(i);
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
