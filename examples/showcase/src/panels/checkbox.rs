use crate::prelude::*;

fn checkbox_row(label: &str, checked: Signal<bool>) -> BoxedWidget {
    let theme = use_theme();
    let is_checked = checked.get();
    let toggle = checked.clone();
    let state_text = if is_checked { "On" } else { "Off" };
    let row_style = Style {
        align_items: Some(AlignItems::Center),
        ..row(theme.spacing_medium)
    };
    let text_style = Style {
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Length(18.0),
        },
        ..Default::default()
    };
    Box::new(jsx! {
        <RawView style={row_style}>
            <Checkbox checked={is_checked} on_click={move || toggle.update(|c| *c = !*c)} />
            <Text align={TextAlign::Start} style={text_style}>{format!("{} — {}", label, state_text)}</Text>
        </RawView>
    })
}

/// The "Checkbox" panel: three checkboxes, each toggling its own boolean
/// `Signal` and reflecting the current state in its label.
#[component]
pub fn CheckboxPanel(
    notifications: Signal<bool>,
    auto_save: Signal<bool>,
    beta_features: Signal<bool>,
) -> BoxedWidget {
    let theme = use_theme();
    let notifications_row = checkbox_row("Notifications", notifications.clone());
    let auto_save_row = checkbox_row("Auto-save", auto_save.clone());
    let beta_row = checkbox_row("Beta features", beta_features);
    let switch_checked = auto_save.get();
    let switch_toggle = auto_save.clone();
    let raw_checked = notifications.get();
    let raw_toggle = notifications.clone();
    let custom_checked = auto_save.get();
    let custom_toggle = auto_save.clone();
    let raw_checkbox = creamui_widgets::RawCheckbox::new(
        18.0,
        raw_checked,
        theme.accent,
        theme.border_strong,
        move || raw_toggle.update(|value| *value = !*value),
    );
    let custom_switch = creamui_widgets::RawSwitch::new(
        custom_checked,
        Color::rgb(0x5c, 0xe1, 0xff),
        Color::rgb(0x16, 0x48, 0x51),
        Color::rgb(0xbf, 0xef, 0xff),
        move || custom_toggle.update(|value| *value = !*value),
    )
    .radii(12.0, 10.0);
    Box::new(jsx! {
        <RawView style={column(section_gap())}>
            <SectionHeader title={"Checkbox".to_owned()} subtitle={"Small preferences, clearly expressed.".to_owned()} />
            <Card gap={theme.spacing_medium}>
                <FieldLabel text={"Preferences".to_owned()} />
                {notifications_row}
                {auto_save_row}
                {beta_row}
            </Card>
            <Card gap={theme.spacing_medium}>
                <FieldLabel text={"Switch presentation".to_owned()} />
                <RawView style={Style { align_items: Some(AlignItems::Center), ..row(theme.spacing_medium) }}>
                    <Switch checked={switch_checked} on_click={Box::new(move || switch_toggle.update(|value| *value = !*value)) as Box<dyn Fn()>} />
                    <Text secondary={true} align={TextAlign::Start}>"The same boolean, shown as a switch instead of a checkbox."</Text>
                </RawView>
            </Card>
            <Card gap={theme.spacing_medium}>
                <FieldLabel text={"Raw and manually styled · linked preferences".to_owned()} />
                <RawView style={Style { align_items: Some(AlignItems::Center), ..row(theme.spacing_medium) }}>
                    {Box::new(raw_checkbox) as BoxedWidget}
                    <Text secondary={true}>"Notifications"</Text>
                    {Box::new(custom_switch) as BoxedWidget}
                    <Text secondary={true}>"Auto-save"</Text>
                </RawView>
            </Card>
        </RawView>
    })
}
