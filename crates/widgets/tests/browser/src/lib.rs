use creamui_core::{Key, KeyInput, Modifiers, Widget};
use creamui_widgets::{FilePicker, TextArea, TextController, TextInput};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! {
    static EDITOR: TextController = TextController::new("héllo");
    static WIDGET: RefCell<Option<Box<dyn Widget>>> = const { RefCell::new(None) };
    static FILE: RefCell<String> = const { RefCell::new(String::new()) };
    static FILE_NAME: RefCell<String> = const { RefCell::new(String::new()) };
    static FILE_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn themed<T>(build: impl FnOnce() -> T) -> T {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(creamui_theme::ThemeProvider::new(
            creamui_theme::Theme::light(),
        ));
        build()
    })
}

#[wasm_bindgen]
pub fn rebuild_editor(multiline: bool) {
    EDITOR.with(|editor| {
        WIDGET.with(|widget| {
            *widget.borrow_mut() = Some(themed(|| {
                if multiline {
                    Box::new(TextArea::controlled(editor)) as Box<dyn Widget>
                } else {
                    Box::new(TextInput::controlled(editor)) as Box<dyn Widget>
                }
            }));
        });
    });
}

#[wasm_bindgen]
pub fn keypress(key: &str, ctrl: bool) {
    let key = match key {
        "Home" => Key::Home,
        "End" => Key::End,
        _ => Key::Char(key.chars().next().unwrap()),
    };
    WIDGET.with(|widget| {
        let handler = widget.borrow().as_ref().unwrap().on_key().unwrap();
        handler(KeyInput {
            key,
            modifiers: Modifiers {
                ctrl,
                ..Default::default()
            },
        });
    });
}

#[wasm_bindgen]
pub fn set_text(text: &str) {
    EDITOR.with(|editor| {
        editor.set_value(text);
        editor.set_cursor(text.len());
        editor.set_selection(creamui_widgets::TextSelection {
            anchor: text.len(),
            focus: text.len(),
        });
    });
}

#[wasm_bindgen]
pub fn editor_text() -> String {
    EDITOR.with(|editor| editor.peek())
}

#[wasm_bindgen]
pub fn select_file() {
    let picker = themed(|| {
        FilePicker::new("", |file| {
            FILE_NAME.with(|name| *name.borrow_mut() = file.name().to_owned());
            assert!(file.path().is_none());
            wasm_bindgen_futures::spawn_local(async move {
                match file.read_bytes().await {
                    Ok(bytes) => {
                        FILE.with(|text| *text.borrow_mut() = String::from_utf8(bytes).unwrap())
                    }
                    Err(error) => FILE_ERROR.with(|text| *text.borrow_mut() = error.to_string()),
                }
            });
        })
        .filter("Text", ["txt", ".md"])
    });
    assert!(picker.focusable());
    picker.on_click().unwrap()();
}

#[wasm_bindgen]
pub fn file_text() -> String {
    FILE.with(|text| text.borrow().clone())
}

#[wasm_bindgen]
pub fn file_name() -> String {
    FILE_NAME.with(|name| name.borrow().clone())
}

#[wasm_bindgen]
pub fn file_error() -> String {
    FILE_ERROR.with(|error| error.borrow().clone())
}
