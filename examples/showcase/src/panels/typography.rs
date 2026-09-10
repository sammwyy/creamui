use crate::prelude::*;

/// A fixed-width, secondary-colored row caption — like [`FieldLabel`] but
/// sized to sit beside its control in a row instead of stacked above it
/// full-width.
fn row_caption(text: &str, width: f32) -> BoxedWidget {
    let style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Length(width),
            height: Dimension::Length(18.0),
        },
        flex_shrink: 0.0,
        ..Default::default()
    };
    Box::new(jsx! {
        <Text secondary={true} align={TextAlign::Start} style={style}>{text.to_owned()}</Text>
    })
}

/// The "Typography" panel: the heading scale (h1-h5) plus one heading per
/// semantic theme color, then every inline text treatment — weight, slant,
/// underline, strikethrough, a blockquote, a preformatted code block, and
/// clickable links.
#[component]
pub fn TypographyPanel(link_clicks: Signal<i32>) -> BoxedWidget {
    let theme = use_theme();
    let row_style = Style {
        align_items: Some(AlignItems::Center),
        ..row(theme.spacing_medium)
    };

    let sizes = [
        (TextSize::Xl, "Xl · h1"),
        (TextSize::Lg, "Lg · h2"),
        (TextSize::Md, "Md · h3"),
        (TextSize::Sm, "Sm · h4"),
        (TextSize::Xs, "Xs · h5"),
    ];
    let mut scale_children: Vec<BoxedWidget> =
        vec![jsx! { <FieldLabel text={"Heading scale".to_owned()} /> }];
    for (size, label) in sizes {
        let caption = row_caption(label, 60.0);
        scale_children.push(Box::new(jsx! {
            <RawView style={row_style.clone()}>
                {caption}
                <Heading size={size}>"Heading"</Heading>
            </RawView>
        }));
    }

    let semantic_colors: [(&str, Color); 7] = [
        ("Primary", theme.text_primary),
        ("Secondary", theme.text_secondary),
        ("Disabled", theme.text_disabled),
        ("Accent", theme.accent),
        ("Danger", theme.danger),
        ("Warning", theme.warning),
        ("Success", theme.success),
    ];
    let mut color_children: Vec<BoxedWidget> =
        vec![jsx! { <FieldLabel text={"Heading · one per semantic color".to_owned()} /> }];
    for (label, color) in semantic_colors {
        let caption = row_caption(label, 78.0);
        color_children.push(Box::new(jsx! {
            <RawView style={row_style.clone()}>
                {caption}
                <Heading color={color}>"The quick brown fox"</Heading>
            </RawView>
        }));
    }

    const SAMPLE: &str = "The quick brown fox jumps over the lazy dog.";
    let mut text_children: Vec<BoxedWidget> =
        vec![jsx! { <FieldLabel text={"Text · weight and decoration".to_owned()} /> }];
    for (label, bold, italic, underline, strikethrough) in [
        ("Normal", false, false, false, false),
        ("Bold", true, false, false, false),
        ("Italic", false, true, false, false),
        ("Underline", false, false, true, false),
        ("Strikethrough", false, false, false, true),
    ] {
        let caption = row_caption(label, 100.0);
        text_children.push(Box::new(jsx! {
            <RawView style={row_style.clone()}>
                {caption}
                <StyledText text={SAMPLE.to_owned()} bold={bold} italic={italic} underline={underline} strikethrough={strikethrough} />
            </RawView>
        }));
    }

    let clicks = link_clicks.get();
    let link_a = link_clicks.clone();
    let link_b = link_clicks.clone();

    Box::new(jsx! {
        <RawView style={column(section_gap())}>
            <SectionHeader title={"Typography".to_owned()} subtitle={"Every heading size and semantic color, plus every inline text treatment.".to_owned()} />
            <Card gap={theme.spacing_medium} children={scale_children} />
            <Card gap={theme.spacing_medium} children={color_children} />
            <Card gap={theme.spacing_medium} children={text_children} />
            <FieldCard label={"Quote".to_owned()} control={jsx!{<Quote text={"Design is not just what it looks like and feels like. Design is how it works.".to_owned()} />}} />
            <FieldCard label={"Pre / code".to_owned()} control={jsx!{<Pre code={"fn main() {\n    println!(\"Hello, CreamUI!\");\n}".to_owned()} />}} />
            <Card gap={theme.spacing_medium}>
                <FieldLabel text={"Links".to_owned()} />
                <RawView style={row(theme.spacing_large)}>
                    <Link text={"Documentation".to_owned()} on_click={Box::new(move || link_a.update(|v| *v += 1)) as Box<dyn Fn()>} />
                    <Link text={"Source on GitHub".to_owned()} on_click={Box::new(move || link_b.update(|v| *v += 1)) as Box<dyn Fn()>} />
                </RawView>
                <Text secondary={true} align={TextAlign::Start}>{format!("{clicks} link clicks")}</Text>
            </Card>
        </RawView>
    })
}
