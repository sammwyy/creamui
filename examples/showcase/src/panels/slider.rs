use crate::prelude::*;

fn slider_row(label: &str, value: Signal<f32>, format: impl Fn(f32) -> String) -> BoxedWidget {
    let theme = use_theme();
    let current = value.get();
    let set = value.clone();
    Box::new(jsx! {
        <RawView style={column(theme.spacing_small)}>
            <Text align={TextAlign::Start} color={theme.text_secondary} style={label_style()}>{label.to_owned()}</Text>
            <Slider value={current} on_change={move |v| set.set(v)} />
            <Text align={TextAlign::Start} color={theme.text_disabled} style={label_style()}>{format(current)}</Text>
        </RawView>
    })
}

/// The "Slider" panel: three independent sliders, each with its live value
/// printed underneath.
#[component]
pub fn SliderPanel(volume: Signal<f32>, brightness: Signal<f32>, zoom: Signal<f32>) -> BoxedWidget {
    let theme = use_theme();
    let volume_row = slider_row("Volume", volume, |v| format!("{:.0}%", v * 100.0));
    let brightness_row = slider_row("Brightness", brightness, |v| format!("{:.0}%", v * 100.0));
    let zoom_row = slider_row("Zoom", zoom.clone(), |v| format!("{:.2}x", 0.5 + v * 1.5));
    let custom_value = zoom.get();
    let set_custom = zoom.clone();
    let custom_slider = creamui_widgets::RawSlider::new(
        Style {
            size: fixed(160.0, 20.0),
            ..Default::default()
        },
        custom_value,
        Color::rgb(0x16, 0x48, 0x51),
        Color::rgb(0x5c, 0xe1, 0xff),
        Color::rgb(0xbf, 0xef, 0xff),
        move |next| set_custom.set(next),
    )
    .track(6.0, 3.0)
    .handle(18.0, 9.0);
    Box::new(jsx! {
        <RawView style={column(section_gap())}>
            <SectionHeader title={"Slider".to_owned()} subtitle={"Fine adjustments with immediate feedback.".to_owned()} />
            <Card gap={theme.spacing_large}>
                {volume_row}
                {brightness_row}
                {zoom_row}
            </Card>
            <Card gap={theme.spacing_medium}>
                <FieldLabel text={"Manual custom · RawSlider (linked to Zoom)".to_owned()} />
                {Box::new(custom_slider) as BoxedWidget}
            </Card>
        </RawView>
    })
}
