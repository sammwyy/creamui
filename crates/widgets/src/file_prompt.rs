use crate::{FilePrompt, RawView, TextController};
use creamui_core::BoxedWidget;
use creamui_reactive::{batch, provide_context, with_context_scope, Signal};
use std::path::PathBuf;
use std::rc::Rc;

pub(crate) struct FileRequest {
    pub title: String,
    pub filters: Vec<(String, Vec<String>)>,
    pub on_change: Rc<dyn Fn(PathBuf)>,
}

/// Persistent state for file path prompts. Keep one controller per window.
#[derive(Clone)]
pub struct FilePickerController {
    pub(crate) request: Signal<Option<Rc<FileRequest>>>,
    path: TextController,
    error: Signal<Option<String>>,
}

impl FilePickerController {
    pub fn new() -> Self {
        let error = Signal::new(None);
        let path = TextController::default();
        let clear_error = error.clone();
        path.on_change(move |_, next| {
            clear_error.set_if_changed(None);
            Some(next.to_owned())
        });
        Self {
            request: Signal::new(None),
            path,
            error,
        }
    }

    /// Builds application content and its modal within the same context scope.
    /// Android `FilePicker::new` reads this controller from that scope.
    pub fn host(&self, build: impl FnOnce() -> BoxedWidget) -> BoxedWidget {
        with_context_scope(|| {
            provide_context(self.clone());
            let content = build();
            let prompt = FilePrompt::new(self);
            Box::new(
                RawView::new(creamui_core::layout::Style {
                    size: creamui_core::layout::Size {
                        width: creamui_core::layout::Dimension::Percent(1.0),
                        height: creamui_core::layout::Dimension::Percent(1.0),
                    },
                    ..crate::layout::column(0.0)
                })
                .child(content)
                .child(Box::new(prompt)),
            ) as BoxedWidget
        })
    }

    pub fn is_open(&self) -> bool {
        self.request.get().is_some()
    }

    pub fn path(&self) -> TextController {
        self.path.clone()
    }

    pub fn error(&self) -> Option<String> {
        self.error.get()
    }

    pub fn cancel(&self) {
        batch(|| {
            self.request.set(None);
            self.error.set_if_changed(None);
        });
        log::debug!("creamui-widgets: file prompt dismissed");
    }

    pub(crate) fn open(&self, value: String, request: FileRequest) {
        batch(|| {
            self.path.set_value(value);
            self.path.set_cursor(self.path.peek().len());
            self.path.set_selection(crate::TextSelection {
                anchor: 0,
                focus: self.path.peek().len(),
            });
            self.error.set_if_changed(None);
            self.request.set(Some(Rc::new(request)));
        });
        log::debug!("creamui-widgets: file prompt opened");
    }

    pub fn submit(&self) {
        let Some(request) = self.request.peek() else {
            return;
        };
        let value = self.path.peek();
        let result = validate_path(&value, &request.filters);
        match result {
            Ok(path) => {
                batch(|| {
                    self.request.set(None);
                    self.error.set_if_changed(None);
                    (request.on_change)(path);
                });
                log::debug!("creamui-widgets: file prompt accepted");
            }
            Err(error) => {
                log::debug!("creamui-widgets: file prompt rejected: {error}");
                self.error.set_if_changed(Some(error));
            }
        }
    }
}

impl Default for FilePickerController {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_path(value: &str, filters: &[(String, Vec<String>)]) -> Result<PathBuf, String> {
    if value.trim().is_empty() {
        return Err("Enter a file path.".into());
    }
    let path = PathBuf::from(value);
    let extensions = filters.iter().flat_map(|(_, extensions)| extensions);
    if !filters.is_empty()
        && !extensions.clone().any(|extension| extension == "*")
        && !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extensions
                    .clone()
                    .any(|allowed| extension.eq_ignore_ascii_case(allowed.trim_start_matches('.')))
            })
    {
        return Err("Choose a file with an allowed extension.".into());
    }
    if !std::fs::metadata(&path)
        .map_err(|error| format!("Cannot inspect this file: {error}"))?
        .is_file()
    {
        return Err("This path does not point to a regular file.".into());
    }
    std::fs::File::open(&path).map_err(|error| format!("Cannot open this file: {error}"))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_validation_rejects_missing_files_directories_and_other_extensions() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest = root.join("Cargo.toml");
        assert!(validate_path(" ", &[]).is_err());
        assert!(validate_path(root.to_str().unwrap(), &[]).is_err());
        assert!(validate_path(
            root.join("absent-file-for-prompt-test").to_str().unwrap(),
            &[]
        )
        .is_err());
        assert!(validate_path(
            manifest.to_str().unwrap(),
            &[("Images".into(), vec!["png".into()])]
        )
        .is_err());
        assert_eq!(
            validate_path(
                manifest.to_str().unwrap(),
                &[("Manifests".into(), vec![".TOML".into()])]
            ),
            Ok(manifest.clone())
        );
        assert_eq!(
            validate_path(
                manifest.to_str().unwrap(),
                &[("All files".into(), vec!["*".into()])]
            ),
            Ok(manifest)
        );
    }
}
