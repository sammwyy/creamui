use crate::prelude::*;
use creamui_core::Styled as _;

/// The "Button" panel: the default themed `Button`, a couple of
/// semantically-colored variants built straight from `RawButton`, and a
/// disabled-looking one, plus a click counter to prove the handlers fire.
#[component]
pub fn ButtonPanel(clicks: Signal<i32>) -> BoxedWidget {
    let theme = use_theme();
    let actions: Vec<BoxedWidget> = [
        ("Continue", ButtonVariant::Primary),
        ("Cancel", ButtonVariant::Secondary),
        ("Learn more", ButtonVariant::Tertiary),
        ("Delete", ButtonVariant::Destructive),
    ]
    .into_iter()
    .map(|(label, variant)| {
        let clicks = clicks.clone();
        jsx! {
            <StyledButton
                variant={variant}
                size={ButtonSize::Md}
                label={label.to_owned()}
                state={ButtonState::Normal}
                on_click={Box::new(move || clicks.update(|c| *c += 1)) as Box<dyn Fn()>}
                disabled={false}
            />
        }
    })
    .collect();

    let sizes: Vec<BoxedWidget> = [
        ("Extra small", ButtonSize::Xs),
        ("Small", ButtonSize::Sm),
        ("Medium", ButtonSize::Md),
        ("Large", ButtonSize::Lg),
    ]
    .into_iter()
    .map(|(label, size)| {
        let clicks = clicks.clone();
        jsx! {
            <StyledButton
                variant={ButtonVariant::Secondary}
                size={size}
                label={label.to_owned()}
                state={ButtonState::Normal}
                on_click={Box::new(move || clicks.update(|c| *c += 1)) as Box<dyn Fn()>}
                disabled={false}
            />
        }
    })
    .collect();

    let raw_clicks = clicks.clone();
    let custom_clicks = clicks.clone();
    let raw_button = creamui_widgets::RawButton::new(
        Style {
            size: fixed(74.0, 36.0),
            align_items: Some(AlignItems::Center),
            justify_content: Some(JustifyContent::Center),
            ..Default::default()
        },
        move || raw_clicks.update(|count| *count += 1),
    )
    .child(Box::new(RawText::new(
        "Raw",
        theme.text_primary,
        theme.typography.body,
    )));
    let custom_button = creamui_widgets::RawButton::new(
        Style {
            size: fixed(126.0, 36.0),
            align_items: Some(AlignItems::Center),
            justify_content: Some(JustifyContent::Center),
            ..Default::default()
        },
        move || custom_clicks.update(|count| *count += 1),
    )
    .background(Color::rgb(0x16, 0x48, 0x51))
    .border(Color::rgb(0x5c, 0xe1, 0xff), 1.0)
    .corner_radius(9.0)
    .child(Box::new(RawText::new(
        "Custom raw",
        Color::rgb(0xbf, 0xef, 0xff),
        theme.typography.body,
    )));

    Box::new(jsx! {
        <RawView style={column(section_gap())}>
            <SectionHeader title={"Buttons".to_owned()} subtitle={"A clear hierarchy, from everyday actions to important decisions.".to_owned()} />
            <Card gap={theme.spacing_medium}>
                <FieldLabel text={"Variants".to_owned()} />
                <RawView style={row(10.)} children={actions} />
            </Card>
            <Card gap={theme.spacing_medium}>
                <FieldLabel text={"One family, four sizes".to_owned()} />
                <RawView style={row(10.)} children={sizes} />
            </Card>
            <Card gap={theme.spacing_medium}>
                <FieldLabel text={"States".to_owned()} />
                <RawView style={row(10.)}>
                    <StyledButton variant={ButtonVariant::Primary} size={ButtonSize::Md} label={"Unavailable".to_owned()} state={ButtonState::Normal} on_click={Box::new(|| {}) as Box<dyn Fn()>} disabled={true} />
                    <StyledButton variant={ButtonVariant::Primary} size={ButtonSize::Md} label={"Working".to_owned()} state={ButtonState::Loading} on_click={Box::new(|| {}) as Box<dyn Fn()>} disabled={false} />
                    <StyledButton variant={ButtonVariant::Primary} size={ButtonSize::Md} label={"Saved".to_owned()} state={ButtonState::Success} on_click={Box::new(|| {}) as Box<dyn Fn()>} disabled={false} />
                </RawView>
                <RawText color={theme.text_secondary} font_size={12.0} align={TextAlign::Start}>{format!("{} actions · Try Tab, then Enter or Space", clicks.get())}</RawText>
            </Card>
            <Card gap={theme.spacing_medium}>
                <FieldLabel text={"Headless and manual styling".to_owned()} />
                <RawView style={row(10.)}>
                    {Box::new(raw_button) as BoxedWidget}
                    {Box::new(custom_button) as BoxedWidget}
                </RawView>
            </Card>
        </RawView>
    })
}
