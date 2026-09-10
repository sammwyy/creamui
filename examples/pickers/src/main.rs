//! Date/time, color, and file pickers, shown as both themed controls and
//! their raw headless building blocks. Run with `cargo run -p pickers`.

use creamui_core::layout::{Dimension, Style};
use creamui_core::{BoxedWidget, Size, StateStyle, TextAlign};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::{run, WindowOptions};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::layout::{column, fixed, padding, row};
use creamui_widgets::{
    ColorPickerController, DateTime, DateTimeController, FilePicker, RawColorPicker,
    RawDateTimePicker, RawFilePicker, RawText,
};

fn heading(theme: &Theme, text: &str) -> BoxedWidget {
    Box::new(
        RawText::new(text, theme.text_primary, theme.typography.section)
            .bold(true)
            .text_align(TextAlign::Start),
    )
}

fn main() {
    let date = DateTimeController::new(DateTime::new(2026, 9, 8, 14, 30));
    let time = DateTimeController::new(DateTime::new(2026, 9, 8, 14, 30));
    let raw_date = Signal::new(DateTime::new(2026, 9, 8, 14, 30));
    let color = Signal::new(Color::rgb(181, 139, 255));
    let color_picker = ColorPickerController::default();
    let raw_color = Signal::new(Color::rgb(105, 218, 166));
    let file = Signal::new(String::new());
    let raw_file_message = Signal::new("No remote asset selected".to_owned());

    run(
        WindowOptions {
            title: "CreamUI — Pickers".into(),
            width: 760,
            height: 620,
            theme: Theme::dark(),
            ..Default::default()
        },
        Theme::dark().surface,
        |_| {},
        move |viewport: Size| -> BoxedWidget {
            let theme = use_theme();
            let selected_color = color.get();
            let selected_raw_color = raw_color.get();
            let raw_value = raw_date.get();
            let file_name = file.get();
            let raw_file_label = raw_file_message.get();
            let date_set = raw_date.clone();
            let color_set = color.clone();
            let raw_color_set = raw_color.clone();
            let file_set = file.clone();
            let raw_file_set = raw_file_message.clone();

            let raw_date_style = Style {
                size: fixed(240., 36.),
                ..Default::default()
            };
            let raw_color_style = Style {
                size: fixed(220., 156.),
                ..Default::default()
            };
            let raw_file_style = Style {
                size: fixed(320., 40.),
                ..Default::default()
            };
            let root_style = Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(viewport.width),
                    height: Dimension::Length(viewport.height),
                },
                ..column(18.)
            };

            let themed_column: BoxedWidget = Box::new(jsx! {
                <RawView style={column(10.)}>
                    <RawText color={theme.text_primary} font_size={theme.typography.section} align={TextAlign::Start}>"Themed · JSX"</RawText>
                    <DateInput controller={&date} popup_width={316.} />
                    <TimeInput controller={&time} minute_step={15} popup_width={250.} />
                    <ColorPicker controller={&color_picker} value={selected_color} on_change={move |next| color_set.set(next)} popup_width={292.} />
                    {Box::new(
                        FilePicker::new(file_name, move |path| file_set.set(path.display().to_string()))
                            .title("Select an image")
                            .filter("Images", ["png", "jpg", "jpeg", "webp"]),
                    ) as BoxedWidget}
                </RawView>
            });

            let raw_date_picker: BoxedWidget = Box::new(
                RawDateTimePicker::new(
                    raw_date_style,
                    raw_value,
                    theme.text_primary,
                    theme.border,
                    move |next| date_set.set(next),
                )
                .background(theme.surface_elevated)
                .hover_style(StateStyle::new().background(theme.surface_hover))
                .border(theme.accent, 1.)
                .corner_radius(2.)
                .focus_style(StateStyle::new().outline(theme.accent, 2.0)),
            );
            let raw_color_picker: BoxedWidget = Box::new(
                RawColorPicker::new(
                    raw_color_style,
                    selected_raw_color,
                    theme.border,
                    theme.text_primary,
                    move |next| raw_color_set.set(next),
                )
                .background(theme.surface_elevated)
                .corner_radius(2.)
                .focus_style(StateStyle::new().outline(theme.accent, 2.0)),
            );
            let raw_file_picker: BoxedWidget = Box::new(
                RawFilePicker::new(
                    raw_file_style,
                    raw_file_label,
                    theme.text_primary,
                    theme.text_disabled,
                    theme.border,
                    move || {
                        raw_file_set.set("Raw picker activated — connect your asset source".into())
                    },
                )
                .background(theme.surface_elevated)
                .hover_style(StateStyle::new().background(theme.surface_hover))
                .corner_radius(2.)
                .focus_style(StateStyle::new().outline(theme.accent, 2.0)),
            );
            let raw_column: BoxedWidget = Box::new(jsx! {
                <RawView style={column(10.)}>
                    {heading(&theme, "Raw / headless")}
                    {raw_date_picker}
                    {raw_color_picker}
                    {raw_file_picker}
                </RawView>
            });

            Box::new(jsx! {
                <RawView style={padding(root_style, 28.)}>
                    {heading(&theme, "Pickers")}
                    <RawView style={row(56.)} children={vec![themed_column, raw_column]} />
                    <Text secondary={true} align={TextAlign::Start}>
                        "Date/time: upper/lower halves increment or decrement. Color: drag in the field or hue strip. FilePicker uses the platform dialog; RawFilePicker only reports activation."
                    </Text>
                </RawView>
            })
        },
    );
}
use creamui_core::Styled as _;
