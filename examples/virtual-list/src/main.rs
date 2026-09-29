use creamui_core::layout::{AlignItems, Dimension, LengthPercentage, Style};
use creamui_core::{BoxedWidget, Size, Styled};
use creamui_render::{run, WindowOptions};
use creamui_theme::{use_theme, Theme};
use creamui_widgets::{RawView, ScrollController, Text, VirtualList, VirtualListState};

const ITEM_COUNT: usize = 100_000;

fn main() {
    let state = VirtualListState::new(ITEM_COUNT, 32.0);
    let scroll = ScrollController::default();
    run(
        WindowOptions {
            title: "CreamUI — Virtual List".into(),
            width: 640,
            height: 480,
            theme: Theme::dark(),
            ..Default::default()
        },
        Theme::dark().surface,
        |_| {},
        move |viewport: Size| -> BoxedWidget {
            let theme = use_theme();
            let style = Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(viewport.width),
                    height: Dimension::Length(viewport.height),
                },
                ..Default::default()
            };
            Box::new(VirtualList::new(
                style,
                state.clone(),
                scroll.clone(),
                viewport.height,
                move |index| {
                    let height = if index % 10 == 0 { 48.0 } else { 32.0 };
                    let background = if index % 2 == 0 {
                        theme.surface_elevated
                    } else {
                        theme.surface
                    };
                    let row_style = Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Percent(1.0),
                            height: Dimension::Length(height),
                        },
                        align_items: Some(AlignItems::Center),
                        padding: creamui_core::layout::Rect {
                            left: LengthPercentage::Length(12.0),
                            right: LengthPercentage::Length(0.0),
                            top: LengthPercentage::Length(0.0),
                            bottom: LengthPercentage::Length(0.0),
                        },
                        ..Default::default()
                    };
                    Box::new(
                        RawView::new(row_style)
                            .background(background)
                            .child(Box::new(Text::new(format!("Row {:05}", index + 1)))),
                    )
                },
            ))
        },
    );
}
