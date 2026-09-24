use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_IMAGE_ID: AtomicU64 = AtomicU64::new(1);

/// Premultiplied RGBA8 pixels with a process-unique identity.
///
/// Renderers key their decoded/uploaded copies on [`RgbaImage::id`], so a
/// clone of the same image never triggers a second upload, while any pixel
/// change produces a new id.
#[derive(Clone)]
pub struct RgbaImage {
    id: u64,
    width: u32,
    height: u32,
    store: Arc<PixelStore>,
}

type Reload = dyn Fn() -> Vec<u8> + Send + Sync;

struct PixelStore {
    decoded: Mutex<Option<Arc<[u8]>>>,
    reload: Option<Box<Reload>>,
}

impl RgbaImage {
    /// Returns `None` unless `pixels` holds exactly `width * height * 4`
    /// premultiplied bytes.
    pub fn new(width: u32, height: u32, pixels: impl Into<Arc<[u8]>>) -> Option<Self> {
        Self::with_store(width, height, pixels.into(), None)
    }

    /// Like [`RgbaImage::new`], but the decoded pixels can be dropped with
    /// [`RgbaImage::discard_pixels`] and are rebuilt by calling `reload`
    /// the next time they are needed. `reload` must reproduce `pixels`.
    pub fn reloadable(
        width: u32,
        height: u32,
        pixels: impl Into<Arc<[u8]>>,
        reload: impl Fn() -> Vec<u8> + Send + Sync + 'static,
    ) -> Option<Self> {
        Self::with_store(width, height, pixels.into(), Some(Box::new(reload)))
    }

    fn with_store(
        width: u32,
        height: u32,
        pixels: Arc<[u8]>,
        reload: Option<Box<Reload>>,
    ) -> Option<Self> {
        let expected = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        (width > 0 && height > 0 && pixels.len() == expected).then(|| Self {
            id: NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed),
            width,
            height,
            store: Arc::new(PixelStore {
                decoded: Mutex::new(Some(pixels)),
                reload,
            }),
        })
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// The premultiplied pixels, reloading them if they were discarded.
    pub fn pixels(&self) -> Arc<[u8]> {
        let mut decoded = self
            .store
            .decoded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        decoded
            .get_or_insert_with(|| {
                let reload = self
                    .store
                    .reload
                    .as_ref()
                    .expect("only reloadable images discard their pixels");
                let pixels: Arc<[u8]> = reload().into();
                assert_eq!(
                    pixels.len(),
                    self.width as usize * self.height as usize * 4,
                    "reloaded image changed size"
                );
                pixels
            })
            .clone()
    }

    /// Drops the decoded pixels of a [`RgbaImage::reloadable`] image, for
    /// every clone sharing them. Returns whether anything was dropped.
    pub fn discard_pixels(&self) -> bool {
        if self.store.reload.is_none() {
            return false;
        }
        self.store
            .decoded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
            .is_some()
    }

    /// Rewrites every pixel in place, assigning a fresh identity. The result
    /// is not reloadable.
    pub fn map_pixels(self, f: impl Fn(&mut [u8])) -> Self {
        let mut pixels = self.pixels().to_vec();
        pixels.chunks_exact_mut(4).for_each(f);
        Self::new(self.width, self.height, pixels).expect("same dimensions as the source image")
    }
}

impl std::fmt::Debug for RgbaImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RgbaImage")
            .field("id", &self.id)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_mismatched_buffers() {
        assert!(RgbaImage::new(2, 2, vec![0u8; 15]).is_none());
        assert!(RgbaImage::new(0, 0, Vec::<u8>::new()).is_none());
        assert!(RgbaImage::new(u32::MAX, u32::MAX, Vec::<u8>::new()).is_none());
    }

    #[test]
    fn clones_share_identity_and_edits_do_not() {
        let image = RgbaImage::new(1, 1, vec![1, 2, 3, 4]).unwrap();
        let clone = image.clone();
        assert_eq!(image.id(), clone.id());
        let edited = clone.map_pixels(|px| px[0] = 9);
        assert_ne!(edited.id(), image.id());
        assert_eq!(&*edited.pixels(), &[9, 2, 3, 4]);
        assert_eq!(&*image.pixels(), &[1, 2, 3, 4]);
    }

    #[test]
    fn only_reloadable_images_discard_and_rebuild_their_pixels() {
        let plain = RgbaImage::new(1, 1, vec![1, 2, 3, 4]).unwrap();
        assert!(!plain.discard_pixels());
        assert_eq!(&*plain.pixels(), &[1, 2, 3, 4]);

        let reloads = Arc::new(AtomicU64::new(0));
        let image = RgbaImage::reloadable(1, 1, vec![5, 6, 7, 8], {
            let reloads = reloads.clone();
            move || {
                reloads.fetch_add(1, Ordering::Relaxed);
                vec![5, 6, 7, 8]
            }
        })
        .unwrap();
        let clone = image.clone();
        assert!(image.discard_pixels());
        assert!(!clone.discard_pixels());
        assert_eq!(&*clone.pixels(), &[5, 6, 7, 8]);
        assert_eq!(&*image.pixels(), &[5, 6, 7, 8]);
        assert_eq!(reloads.load(Ordering::Relaxed), 1);
    }
}
