use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct SelectedFile {
    name: String,
    source: FileSource,
}

#[derive(Clone, Debug)]
enum FileSource {
    Local(PathBuf),
    #[cfg(target_arch = "wasm32")]
    Browser(web_sys::File),
}

impl SelectedFile {
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        Self {
            name,
            source: FileSource::Local(path),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn from_browser_file(file: web_sys::File) -> Self {
        Self {
            name: file.name(),
            source: FileSource::Browser(file),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> Option<&Path> {
        match &self.source {
            FileSource::Local(path) => Some(path),
            #[cfg(target_arch = "wasm32")]
            FileSource::Browser(_) => None,
        }
    }

    pub async fn read_bytes(&self) -> std::io::Result<Vec<u8>> {
        match &self.source {
            FileSource::Local(path) => read_path(path).await,
            #[cfg(target_arch = "wasm32")]
            FileSource::Browser(file) => {
                let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
                    .await
                    .map_err(|error| {
                        std::io::Error::other(format!("Cannot read browser file: {error:?}"))
                    })?;
                Ok(js_sys::Uint8Array::new(&buffer).to_vec())
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn read_path(path: &Path) -> std::io::Result<Vec<u8>> {
    let path = path.to_owned();
    let (sender, receiver) = futures_channel::oneshot::channel();
    std::thread::Builder::new()
        .name("creamui-file-read".into())
        .spawn(move || {
            let _ = sender.send(std::fs::read(path));
        })?;
    receiver
        .await
        .map_err(|_| std::io::Error::other("File read worker stopped"))?
}

#[cfg(target_arch = "wasm32")]
async fn read_path(_: &Path) -> std::io::Result<Vec<u8>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Local file paths are unavailable in the browser",
    ))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn native_selections_keep_the_path_and_defer_reading() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let file = SelectedFile::from_path(&path);
        assert_eq!(file.name(), "Cargo.toml");
        assert_eq!(file.path(), Some(path.as_path()));
        assert_eq!(
            pollster::block_on(file.read_bytes()).unwrap(),
            std::fs::read(path).unwrap()
        );
        let missing = SelectedFile::from_path("absent-selection-test-file");
        assert_eq!(
            pollster::block_on(missing.read_bytes()).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    }
}
