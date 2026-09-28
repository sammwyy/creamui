use crate::ImageData;
use creamui_core::runtime::{ImageFit, Mutation, RuntimeNodeId};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::JoinHandle;
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

enum LoadJob {
    Bytes(ResourceId, Vec<u8>),
    Path(ResourceId, PathBuf),
    Stop,
}

impl ResourceReady {
    /// Converts a successful asynchronous decode into the runtime mutation
    /// that replaces an image node's content. Failed requests remain errors
    /// and do not mutate the tree.
    pub fn into_mutation(self, node: RuntimeNodeId, fit: ImageFit) -> Result<Mutation, String> {
        match self.outcome {
            LoadOutcome::Ready(data) => Ok(Mutation::SetImage {
                node,
                content: creamui_core::runtime::ImageContent::Decoded(data.image().clone()),
                fit,
            }),
            LoadOutcome::Failed(error) => Err(error),
        }
    }
}

/// Decodes image bytes/files on a spawned thread instead of blocking the
/// caller. Delivers a [`ResourceReady`] message only — turning one into a
/// runtime mutation and texture upload is left to the caller.
pub struct BackgroundImageLoader {
    next_id: u64,
    jobs: SyncSender<LoadJob>,
    receiver: Receiver<ResourceReady>,
    workers: Vec<JoinHandle<()>>,
}

impl BackgroundImageLoader {
    pub fn new() -> Self {
        const QUEUE_CAPACITY: usize = 32;
        let (jobs, job_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (sender, receiver) = mpsc::channel();
        let job_receiver = std::sync::Arc::new(std::sync::Mutex::new(job_receiver));
        let worker_count = std::thread::available_parallelism()
            .map_or(2, |parallelism| parallelism.get().clamp(1, 4));
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let jobs = job_receiver.clone();
            let sender = sender.clone();
            workers.push(std::thread::spawn(move || loop {
                let job = jobs
                    .lock()
                    .expect("background image job queue lock poisoned")
                    .recv();
                let Ok(job) = job else { break };
                let (id, outcome) = match job {
                    LoadJob::Bytes(id, bytes) => (
                        id,
                        ImageData::from_bytes(&bytes)
                            .map(LoadOutcome::Ready)
                            .unwrap_or_else(|err| LoadOutcome::Failed(err.to_string())),
                    ),
                    LoadJob::Path(id, path) => (
                        id,
                        ImageData::from_path(&path)
                            .map(LoadOutcome::Ready)
                            .unwrap_or_else(|err| LoadOutcome::Failed(err.to_string())),
                    ),
                    LoadJob::Stop => break,
                };
                let _ = sender.send(ResourceReady { id, outcome });
            }));
        }
        BackgroundImageLoader {
            next_id: 0,
            jobs,
            receiver,
            workers,
        }
    }

    pub fn load_bytes(&mut self, bytes: Vec<u8>) -> ResourceId {
        let id = self.issue_id();
        self.jobs
            .send(LoadJob::Bytes(id, bytes))
            .expect("background image workers have stopped");
        id
    }

    pub fn load_path(&mut self, path: impl Into<PathBuf>) -> ResourceId {
        let id = self.issue_id();
        self.jobs
            .send(LoadJob::Path(id, path.into()))
            .expect("background image workers have stopped");
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

impl Drop for BackgroundImageLoader {
    fn drop(&mut self) {
        for _ in &self.workers {
            let _ = self.jobs.send(LoadJob::Stop);
        }
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
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
    fn ready_image_becomes_a_runtime_mutation() {
        use creamui_core::runtime::{ImageContent, NodeKind, Runtime};

        let mut runtime = Runtime::new();
        let node = runtime.transaction().create_node(NodeKind::Image(
            creamui_core::runtime::ImageNode::source("pending"),
        ));
        let mut loader = BackgroundImageLoader::new();
        let id = loader.load_bytes(tiny_png_bytes());
        let ready = loader
            .recv_timeout(Duration::from_secs(5))
            .expect("decode completed within the timeout");
        assert_eq!(ready.id, id);
        let mutation = ready
            .into_mutation(node, ImageFit::Contain)
            .expect("image should decode");
        runtime.transaction().apply(mutation);

        let runtime_node = runtime.get(node).unwrap();
        let NodeKind::Image(image) = &runtime_node.kind else {
            panic!("mutation should retain an image node");
        };
        assert_eq!(image.fit, ImageFit::Contain);
        assert!(matches!(image.content, ImageContent::Decoded(_)));
        assert!(runtime_node
            .dirty
            .contains(creamui_core::runtime::DirtyFlags::PAINT));
    }

    #[test]
    fn successive_requests_get_distinct_ids() {
        let mut loader = BackgroundImageLoader::new();
        let a = loader.load_bytes(tiny_png_bytes());
        let b = loader.load_bytes(tiny_png_bytes());
        assert_ne!(a, b);
    }
}
