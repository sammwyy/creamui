//! PNG, JPEG, and WebP rendering with square, rounded, and circular crops.
//! Run with `cargo run -p images`.

use creamui_core::layout::{Dimension, Style};
use creamui_core::{BoxedWidget, Size, TextAlign};
use creamui_image::{Image, ImageData, ImageFit};
use creamui_macros::jsx;
use creamui_render::{run, WindowOptions};
use creamui_theme::{use_theme, Theme};
use creamui_widgets::layout::{column, fixed, padding, row};
use creamui_widgets::{RawText, Surface, SurfaceRole, TextSize};

fn image_style(width: f32, height: f32) -> Style {
    Style {
        size: fixed(width, height),
        flex_shrink: 0.,
        ..Default::default()
    }
}

fn image_card(title: &str, description: &str, image: Image) -> BoxedWidget {
    let theme = use_theme();
    Box::new(
        Surface::new(
            SurfaceRole::Inset,
            padding(
                Style {
                    flex_grow: 1.,
                    size: creamui_core::layout::Size {
                        width: Dimension::Auto,
                        height: Dimension::Auto,
                    },
                    ..column(theme.spacing_medium)
                },
                theme.spacing_large,
            ),
        )
        .child(Box::new(
            RawText::new(title, theme.text_primary, theme.typography.section)
                .bold(true)
                .text_align(TextAlign::Start),
        ))
        .child(Box::new(image))
        .child(Box::new(jsx! {
            <Text secondary={true} align={TextAlign::Start}>{description.to_owned()}</Text>
        })),
    )
}

fn main() {
    let png = ImageData::from_bytes(include_bytes!("../assets/iridescent.png"))
        .expect("bundled PNG should decode");
    let jpeg = ImageData::from_bytes(include_bytes!("../assets/still-life.jpg"))
        .expect("bundled JPEG should decode");
    let webp = ImageData::from_bytes(include_bytes!("../assets/botanical.webp"))
        .expect("bundled WebP should decode");

    run(
        WindowOptions {
            title: "CreamUI — Images".into(),
            width: 1080,
            height: 560,
            theme: Theme::dark(),
            ..Default::default()
        },
        Theme::dark().surface,
        |_| {},
        move |viewport: Size| -> BoxedWidget {
            let theme = use_theme();
            let root = Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(viewport.width),
                    height: Dimension::Length(viewport.height),
                },
                ..column(theme.spacing_large)
            };
            let png_card = image_card(
                "PNG · square",
                "Cover fit without clipping the corners.",
                Image::new(png.clone())
                    .layout(image_style(190., 190.))
                    .fit(ImageFit::Cover),
            );
            let jpeg_card = image_card(
                "JPEG · rounded",
                "A 4:3 crop with the active theme's radius.",
                Image::new(jpeg.clone())
                    .layout(image_style(230., 172.))
                    .fit(ImageFit::Cover)
                    .corner_radius(theme.card_radius),
            );
            let webp_card = image_card(
                "WebP · circle",
                "A square crop clipped into a complete circle.",
                Image::new(webp.clone())
                    .layout(image_style(190., 190.))
                    .fit(ImageFit::Cover)
                    .corner_radius(95.),
            );

            Box::new(jsx! {
                <RawView style={padding(root, 32.)}>
                    <Heading size={TextSize::Xl}>"Images"</Heading>
                    <Text secondary={true} align={TextAlign::Start}>
                        "One decoded asset type, three file formats, and three independent shapes."
                    </Text>
                    <RawView style={row(theme.spacing_large)} children={vec![png_card, jpeg_card, webp_card]} />
                </RawView>
            })
        },
    );
}
use creamui_core::Styled as _;
