//! Two grid patterns: an explicit Bento dashboard and an adaptive gallery.
//!
//! The first card occupies a 2×2 area while the side cards target individual
//! cells. The gallery derives its column count from the current viewport, so
//! cards fill a row until the next one would be narrower than its minimum.

use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_render::{run, WindowOptions};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::layout::{Align, GridItem, Justify, StyleExt, Track};
use creamui_widgets::RawText;

const PAGE_PADDING: f32 = 24.0;
const GRID_GAP: f32 = 14.0;
const MIN_GALLERY_CARD_WIDTH: f32 = 176.0;

fn card(title: &'static str, description: &'static str, accent: Color) -> BoxedWidget {
    let theme = use_theme();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} fill={true} gap={10.0} padding={18.0} background={theme.surface_elevated} corner_radius={theme.card_radius}>
            {Box::new(RawText::new(title, accent, 18.0).bold(true)) as BoxedWidget}
            <RawText color={theme.text_secondary} font_size={13.0} style={creamui_core::layout::Style::default().grow(1.0)}>{description}</RawText>
        </Flex>
    })
}

fn gallery_card(title: &'static str, description: &'static str, accent: Color) -> BoxedWidget {
    let theme = use_theme();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} full_width={true} gap={6.0} padding={14.0} background={theme.surface_elevated} corner_radius={theme.card_radius}>
            {Box::new(RawText::new(title, accent, 15.0).bold(true)) as BoxedWidget}
            <RawText color={theme.text_secondary} font_size={12.0}>{description}</RawText>
        </Flex>
    })
}

fn main() {
    run(
        WindowOptions {
            title: "CreamUI — Grid layout".into(),
            width: 900,
            height: 620,
            theme: Theme::dark(),
            ..Default::default()
        },
        Theme::dark().surface,
        |_| {},
        move |viewport: Size| -> BoxedWidget {
            let theme = use_theme();
            let content_width = (viewport.width - PAGE_PADDING * 2.0).max(0.0);
            let gallery_columns = ((content_width + GRID_GAP) / (MIN_GALLERY_CARD_WIDTH + GRID_GAP))
                .floor()
                .max(1.0) as usize;

            let header = Box::new(jsx! {
                <Flex align={Align::Center} justify={Justify::Between}>
                    <Text font_size={24.0}>"Grid, placed deliberately"</Text>
                    <RawText color={theme.accent} font_size={13.0} style={creamui_core::layout::Style::default().padding_all(8.0)}>"display: grid"</RawText>
                </Flex>
            }) as BoxedWidget;

            let bento = Box::new(jsx! {
                <Grid size={(content_width, 220.0)} gap={GRID_GAP} template_columns={[Track::fr(1.0), Track::fr(1.0), Track::fr(1.0)]} template_rows={[Track::fr(1.0), Track::fr(1.0)]}>
                    <GridItem column={1} row={1} column_span={2} row_span={2}>
                        {card(
                            "Overview",
                            "This item starts at column 1, row 1 and spans two columns and two rows.",
                            theme.accent,
                        )}
                    </GridItem>
                    <GridItem column={3} row={1}>
                        {card(
                            "Cell",
                            "An item can target one explicit cell.",
                            Color::rgb(0x8b, 0x5c, 0xf6),
                        )}
                    </GridItem>
                    <GridItem column={3} row={2}>
                        {card(
                            "Tracks",
                            "Three proportional fr columns share the available width.",
                            Color::rgb(0x22, 0xc5, 0x5e),
                        )}
                    </GridItem>
                </Grid>
            }) as BoxedWidget;

            let gallery_heading = Box::new(jsx! {
                <Flex align={Align::Center} justify={Justify::Between}>
                    <Text font_size={16.0}>"Adaptive gallery"</Text>
                    <RawText color={theme.text_disabled} font_size={12.0}>
                        {format!("{gallery_columns} columns · min {MIN_GALLERY_CARD_WIDTH:.0}px")}
                    </RawText>
                </Flex>
            }) as BoxedWidget;

            let gallery_items: Vec<BoxedWidget> = vec![
                Box::new(GridItem::new().min_width(0.0).child(gallery_card(
                    "Auto flow",
                    "Items fill rows from left to right.",
                    theme.accent,
                ))),
                Box::new(GridItem::new().min_width(0.0).child(gallery_card(
                    "Minimum",
                    "Columns are added only when a card stays readable.",
                    Color::rgb(0x8b, 0x5c, 0xf6),
                ))),
                Box::new(GridItem::new().min_width(0.0).child(gallery_card(
                    "Stretch",
                    "Each fr track shares the remaining width.",
                    Color::rgb(0x22, 0xc5, 0x5e),
                ))),
                Box::new(GridItem::new().min_width(0.0).child(gallery_card(
                    "Resize",
                    "Resize the window to recompute the column count.",
                    Color::rgb(0xf5, 0x9e, 0x0b),
                ))),
                Box::new(GridItem::new().min_width(0.0).child(gallery_card(
                    "Next row",
                    "Extra cards continue below the current row.",
                    Color::rgb(0xec, 0x48, 0x99),
                ))),
                Box::new(GridItem::new().min_width(0.0).child(gallery_card(
                    "No overlap",
                    "Card content uses a vertical flex flow.",
                    Color::rgb(0x06, 0xb6, 0xd4),
                ))),
            ];
            let gallery = Box::new(
                jsx! { <Grid grow={1.0} columns={gallery_columns} gap={GRID_GAP} children={gallery_items} /> },
            ) as BoxedWidget;

            let footer = Box::new(jsx! {
                <Flex justify={Justify::End}>
                    <RawText color={theme.text_disabled} font_size={12.0}>"Bento · Auto-flow · fr tracks · StyleExt"</RawText>
                </Flex>
            }) as BoxedWidget;

            Box::new(jsx! {
                <Flex direction={FlexDirection::Column} size={(viewport.width, viewport.height)} gap={GRID_GAP} padding={PAGE_PADDING} background={theme.surface}>
                    {header}
                    {bento}
                    {gallery_heading}
                    {gallery}
                    {footer}
                </Flex>
            })
        },
    );
}
use creamui_core::Styled as _;
