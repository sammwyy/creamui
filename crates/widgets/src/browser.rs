use crate::SelectedFile;
use futures_channel::oneshot;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::{Event, HtmlInputElement};

struct FileDialog {
    input: HtmlInputElement,
    change: Closure<dyn FnMut(Event)>,
    cancel: Closure<dyn FnMut(Event)>,
}

impl Drop for FileDialog {
    fn drop(&mut self) {
        for (event, handler) in [("change", &self.change), ("cancel", &self.cancel)] {
            if let Err(error) = self
                .input
                .remove_event_listener_with_callback(event, handler.as_ref().unchecked_ref())
            {
                log::warn!(
                    "creamui-widgets: cannot detach browser file {event} handler: {error:?}"
                );
            }
        }
        self.input.remove();
    }
}

fn file_dialog(
    filters: &[(String, Vec<String>)],
) -> Result<(FileDialog, oneshot::Receiver<Option<web_sys::File>>), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("Browser document is unavailable"))?;
    let input: HtmlInputElement = document.create_element("input")?.dyn_into()?;
    input.set_type("file");
    input.set_hidden(true);
    let extensions: std::collections::BTreeSet<_> = filters
        .iter()
        .flat_map(|(_, extensions)| extensions.iter())
        .collect();
    if !extensions.iter().any(|extension| extension.as_str() == "*") {
        input.set_accept(
            &extensions
                .iter()
                .map(|extension| format!(".{}", extension.trim_start_matches('.')))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    let (sender, receiver) = oneshot::channel();
    let sender = Rc::new(RefCell::new(Some(sender)));
    let changed_input = input.clone();
    let changed_sender = sender.clone();
    let change = Closure::new(move |_: Event| {
        if let Some(sender) = changed_sender.borrow_mut().take() {
            let _ = sender.send(changed_input.files().and_then(|files| files.get(0)));
        }
    });
    let cancel = Closure::new(move |_: Event| {
        if let Some(sender) = sender.borrow_mut().take() {
            let _ = sender.send(None);
        }
    });
    let dialog = FileDialog {
        input,
        change,
        cancel,
    };
    dialog
        .input
        .add_event_listener_with_callback("change", dialog.change.as_ref().unchecked_ref())?;
    dialog
        .input
        .add_event_listener_with_callback("cancel", dialog.cancel.as_ref().unchecked_ref())?;
    document
        .body()
        .ok_or_else(|| JsValue::from_str("Browser document body is unavailable"))?
        .append_child(&dialog.input)?;
    dialog.input.show_picker()?;
    Ok((dialog, receiver))
}

pub(crate) fn pick_file(filters: &[(String, Vec<String>)], callback: Rc<dyn Fn(SelectedFile)>) {
    match file_dialog(filters) {
        Ok((dialog, receiver)) => wasm_bindgen_futures::spawn_local(async move {
            let result = receiver.await;
            drop(dialog);
            match result {
                Ok(Some(file)) => callback(SelectedFile::from_browser_file(file)),
                Ok(None) => log::debug!("creamui-widgets: browser file selection canceled"),
                Err(error) => {
                    log::warn!("creamui-widgets: browser file selection stopped: {error}")
                }
            }
        }),
        Err(error) => log::warn!("creamui-widgets: cannot open browser file picker: {error:?}"),
    }
}
