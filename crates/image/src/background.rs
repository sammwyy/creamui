use crate::ImageData;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(u64);

pub enum LoadOutcome {
    Ready(ImageData),
    Failed(String),
}

pub struct ResourceReady {
    pub id: ResourceId,
    pub outcome: LoadOutcome,
}

/// Decodes image bytes/files on a spawned thread instead of blocking the
/// caller. Delivers a [`ResourceReady`] message only — turning one into a
/// runtime mutation and texture upload is left to the caller.
pub struct BackgroundImageLoader {
    next_id: u64,
    sender: Sender<ResourceReady>,
    receiver: Receiver<ResourceReady>,
}

impl BackgroundImageLoader {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        BackgroundImageLoader {
            next_id: 0,
            sender,
            receiver,
        }
    }

    pub fn load_bytes(&mut self, bytes: Vec<u8>) -> ResourceId {
        let id = self.issue_id();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let outcome = match ImageData::from_bytes(&bytes) {
                Ok(data) => LoadOutcome::Ready(data),
                Err(err) => LoadOutcome::Failed(err.to_string()),
            };
            let _ = sender.send(ResourceReady { id, outcome });
        });
        id
    }

    pub fn load_path(&mut self, path: impl Into<PathBuf>) -> ResourceId {
        let id = self.issue_id();
        let sender = self.sender.clone();
        let path = path.into();
        std::thread::spawn(move || {
            let outcome = match ImageData::from_path(&path) {
                Ok(data) => LoadOutcome::Ready(data),
                Err(err) => LoadOutcome::Failed(err.to_string()),
            };
            let _ = sender.send(ResourceReady { id, outcome });
        });
        id
    }

    /// Every decode that finished since the last call, without blocking.
    pub fn poll_ready(&self) -> Vec<ResourceReady> {
        self.receiver.try_iter().collect()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<ResourceReady> {
        self.receiver.recv_timeout(timeout).ok()
    }

    fn issue_id(&mut self) -> ResourceId {
        let id = ResourceId(self.next_id);
        self.next_id += 1;
        id
    }
}

impl Default for BackgroundImageLoader {
    fn default() -> Self {
        BackgroundImageLoader::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn tiny_png_bytes() -> Vec<u8> {
        let pixel = image_rs::Rgba([255, 0, 0, 255]);
        let img = image_rs::RgbaImage::from_pixel(2, 2, pixel);
        let mut bytes = Vec::new();
        image_rs::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut bytes), image_rs::ImageFormat::Png)
            .expect("encoding a tiny in-memory PNG cannot fail");
        bytes
    }

    fn sizable_png_bytes() -> Vec<u8> {
        let pixel = image_rs::Rgba([10, 20, 30, 255]);
        let img = image_rs::RgbaImage::from_pixel(1200, 1200, pixel);
        let mut bytes = Vec::new();
        image_rs::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut bytes), image_rs::ImageFormat::Png)
            .expect("encoding a large in-memory PNG cannot fail");
        bytes
    }

    #[test]
    fn load_bytes_returns_before_the_decode_it_spawned_finishes() {
        let png = sizable_png_bytes();
        let decode_cost = {
            let start = std::time::Instant::now();
            ImageData::from_bytes(&png).expect("decodes");
            start.elapsed()
        };

        let mut loader = BackgroundImageLoader::new();
        let start = std::time::Instant::now();
        loader.load_bytes(png);
        let call_cost = start.elapsed();

        assert!(
            call_cost < decode_cost,
            "load_bytes ({call_cost:?}) should return well before a synchronous decode \
             finishes ({decode_cost:?})"
        );
    }

    #[test]
    fn load_bytes_delivers_a_resource_ready_message() {
        let mut loader = BackgroundImageLoader::new();
        let id = loader.load_bytes(tiny_png_bytes());

        let ready = loader
            .recv_timeout(Duration::from_secs(5))
            .expect("decode completed within the timeout");
        assert_eq!(ready.id, id);
        match ready.outcome {
            LoadOutcome::Ready(data) => assert_eq!((data.width(), data.height()), (2, 2)),
            LoadOutcome::Failed(err) => panic!("expected a successful decode, got {err}"),
        }
    }

    #[test]
    fn load_bytes_reports_failure_for_invalid_data() {
        let mut loader = BackgroundImageLoader::new();
        loader.load_bytes(vec![0, 1, 2, 3]);

        let ready = loader
            .recv_timeout(Duration::from_secs(5))
            .expect("decode completed within the timeout");
        assert!(matches!(ready.outcome, LoadOutcome::Failed(_)));
    }

    #[test]
    fn poll_ready_drains_every_completed_request() {
        let mut loader = BackgroundImageLoader::new();
        let a = loader.load_bytes(tiny_png_bytes());
        let b = loader.load_bytes(tiny_png_bytes());

        std::thread::sleep(Duration::from_millis(200));
        let ready = loader.poll_ready();
        let ids: Vec<_> = ready.iter().map(|r| r.id).collect();
        assert_eq!(ready.len(), 2);
        assert!(ids.contains(&a));
        assert!(ids.contains(&b));
    }

    #[test]
    fn poll_ready_is_empty_with_no_pending_requests() {
        let loader = BackgroundImageLoader::new();
        assert!(loader.poll_ready().is_empty());
    }

    #[test]
    fn successive_requests_get_distinct_ids() {
        let mut loader = BackgroundImageLoader::new();
        let a = loader.load_bytes(tiny_png_bytes());
        let b = loader.load_bytes(tiny_png_bytes());
        assert_ne!(a, b);
    }
}
