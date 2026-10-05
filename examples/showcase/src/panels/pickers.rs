use crate::prelude::*;

/// Date/time, color, and file pickers live together because each returns a
/// value chosen from a structured external domain rather than free text.
/// Picker controllers retain their state across reactive rebuilds.
#[component]
pub fn PickersPanel(
    date_time: DateTimeController,
    color: Signal<Color>,
    color_picker: ColorPickerController,
    file: Signal<String>,
) -> BoxedWidget {
    let theme = use_theme();
    let selected_color = color.get();
    let file_label = file.get();
    let set_color = color.clone();
    let set_file = file.clone();
    let color_label = format!(
        "#{:02X}{:02X}{:02X}",
        selected_color.r, selected_color.g, selected_color.b
    );
    let file_caption = if file_label.is_empty() {
        "No file selected yet".to_owned()
    } else {
        file_label.clone()
    };
    let file_control: BoxedWidget = Box::new(jsx! {
        <RawView style={column(theme.spacing_small)}>
            <FilePicker
                value={file_label}
                on_change={Box::new(move |file: creamui_widgets::SelectedFile| set_file.set(file.path().map_or_else(|| file.name().to_owned(), |path| path.display().to_string()))) as Box<dyn Fn(creamui_widgets::SelectedFile)>}
                title={"Choose an asset".to_owned()}
                filter_label={"Images".to_owned()}
                filter_extensions={vec!["png".to_owned(), "jpg".to_owned(), "jpeg".to_owned(), "webp".to_owned()]}
            />
            <Text secondary={true} align={TextAlign::Start}>{file_caption}</Text>
        </RawView>
    });
    Box::new(jsx! {
        <RawView style={column(section_gap())}>
            <SectionHeader
                title={"Pickers".to_owned()}
                subtitle={"Structured values, controlled by the application and styled from the active theme.".to_owned()}
            />
            <FieldCard
                label={"Date & time · arrows or upper/lower portions adjust it".to_owned()}
                control={Box::new(jsx!{<DateTimePicker controller={&date_time} />}) as BoxedWidget}
            />
            <CardRow>
                <FieldCard
                    label={format!("Color · {color_label}")}
                    control={Box::new(jsx!{<ColorPicker controller={&color_picker} value={selected_color} on_change={move |next| set_color.set(next)} />}) as BoxedWidget}
                />
                <FieldCard label={if cfg!(target_os = "android") { "File · local path prompt".to_owned() } else if cfg!(target_arch = "wasm32") { "File · unavailable in the browser".to_owned() } else { "File · native system dialog".to_owned() }} control={file_control} />
            </CardRow>
        </RawView>
    })
}
