use crate::prelude::*;
use creamui_core::Styled;

#[component]
pub fn ThemingPanel(
    mode: Signal<ThemeMode>,
    accent_index: Signal<usize>,
    raw_clicks: Signal<usize>,
) -> BoxedWidget {
    let theme = use_theme();
    let switch_mode = mode.clone();
    let raw_tap = raw_clicks.clone();
    Box::new(
        RawView::new(column(section_gap()))
            .child(SectionHeader(SectionHeaderProps {
                title: "Theming".into(),
                subtitle: "Compare a theme-driven control with one styled explicitly; change the active theme without rebuilding application state.".into(),
            }))
            .child(Box::new(
                creamui_widgets::Surface::new(
                    SurfaceRole::Inset,
                    padding(column(theme.spacing_medium), theme.spacing_medium),
                )
                .child(Box::new(RawText::new(
                    format!("Active palette: {} · accent {}", match mode.get() {
                        ThemeMode::Light => "Light",
                        ThemeMode::Dark => "Dark",
                        ThemeMode::Midnight => "Midnight",
                    }, ACCENTS[accent_index.get()].0),
                    theme.text_secondary,
                    theme.typography.body,
                )))
                .child(Box::new(
                    RawView::new(row(theme.spacing_small))
                        .child(Box::new(Button::new("Themed button", move || {
                            switch_mode.update(|mode| *mode = if *mode == ThemeMode::Light {
                                ThemeMode::Dark
                            } else {
                                ThemeMode::Light
                            })
                        })))
                        .child(Box::new(
                            creamui_widgets::RawButton::new(
                                Style::default().padding_all(theme.spacing_small),
                                move || raw_tap.update(|clicks| *clicks += 1),
                            )
                            .background(Color::rgb(0x32, 0x50, 0x6b))
                            .corner_radius(6.0)
                            .child(Box::new(RawText::new(
                                "Raw button",
                                Color::rgb(0xff, 0xff, 0xff),
                                theme.typography.body,
                            ))),
                        )),
                ))
                .child(Box::new(RawText::new(
                    format!("Raw clicks: {}", raw_clicks.get()),
                    theme.text_secondary,
                    theme.typography.caption,
                ))),
            )),
    )
}

#[component]
pub fn HooksPanel(counter: Signal<i32>, enabled: Signal<bool>) -> BoxedWidget {
    let theme = use_theme();
    let increase = counter.clone();
    let decrease = counter.clone();
    let toggle = enabled.clone();
    Box::new(
        RawView::new(column(section_gap()))
            .child(SectionHeader(SectionHeaderProps {
                title: "Hooks & state".into(),
                subtitle: "Signals keep state across page switches; use_theme supplies live colors and spacing when the palette changes.".into(),
            }))
            .child(Box::new(
                creamui_widgets::Surface::new(
                    SurfaceRole::Inset,
                    padding(column(theme.spacing_medium), theme.spacing_medium),
                )
                .child(Box::new(RawText::new(
                    format!("Counter: {} · {}", counter.get(), if enabled.get() { "Enabled" } else { "Paused" }),
                    theme.text_primary,
                    theme.typography.body,
                )))
                .child(Box::new(
                    RawView::new(row(theme.spacing_small))
                        .child(Box::new(Button::secondary(ButtonSize::Sm, "−", move || {
                            decrease.update(|value| *value -= 1)
                        })))
                        .child(Box::new(Button::new("+", move || {
                            increase.update(|value| *value += 1)
                        })))
                        .child(Box::new(Button::secondary(ButtonSize::Sm, "Toggle", move || {
                            toggle.update(|value| *value = !*value)
                        }))),
                )),
            )),
    )
}

#[component]
pub fn EventsPanel(
    clicks: Signal<usize>,
    last_event: Signal<String>,
    input: TextController,
) -> BoxedWidget {
    let theme = use_theme();
    let count = clicks.clone();
    let position = last_event.clone();
    let keyboard = last_event.clone();
    let focus = last_event.clone();
    let button = creamui_widgets::RawButton::new(
        Style::default().padding_all(theme.spacing_small),
        move || count.update(|value| *value += 1),
    )
    .with_click_position(move |point| {
        position.set(format!("Pointer click at {:.0}, {:.0}", point.x, point.y))
    })
    .background(theme.surface_hover)
    .corner_radius(theme.radius_small)
    .child(Box::new(RawText::new(
        "Click or press Enter",
        theme.text_primary,
        theme.typography.body,
    )));
    let text_input = TextInput::controlled(&input)
        .placeholder("Focus and type here")
        .on_key_press(move |key| keyboard.set(format!("Key: {:?}", key.key)))
        .on_focus_change(move |focused| {
            focus.set(
                if focused {
                    "Input focused"
                } else {
                    "Input blurred"
                }
                .into(),
            )
        });
    Box::new(
        RawView::new(column(section_gap()))
            .child(SectionHeader(SectionHeaderProps {
                title: "Events".into(),
                subtitle:
                    "Pointer, keyboard and focus callbacks update independent reactive state."
                        .into(),
            }))
            .child(Box::new(
                creamui_widgets::Surface::new(
                    SurfaceRole::Inset,
                    padding(column(theme.spacing_medium), theme.spacing_medium),
                )
                .child(Box::new(button))
                .child(Box::new(text_input))
                .child(Box::new(RawText::new(
                    format!("Clicks: {} · {}", clicks.get(), last_event.get()),
                    theme.text_secondary,
                    theme.typography.caption,
                ))),
            )),
    )
}
