use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
    pixels: Arc<[u8]>,
}

impl RgbaImage {
    /// Returns `None` unless `pixels` holds exactly `width * height * 4`
    /// premultiplied bytes.
    pub fn new(width: u32, height: u32, pixels: impl Into<Arc<[u8]>>) -> Option<Self> {
        let pixels = pixels.into();
        let expected = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        (width > 0 && height > 0 && pixels.len() == expected).then(|| Self {
            id: NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed),
            width,
            height,
            pixels,
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

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Rewrites every pixel in place, assigning a fresh identity.
    pub fn map_pixels(mut self, f: impl Fn(&mut [u8])) -> Self {
        Arc::make_mut(&mut self.pixels)
            .chunks_exact_mut(4)
            .for_each(f);
        self.id = NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed);
        self
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
        assert_eq!(edited.pixels(), &[9, 2, 3, 4]);
        assert_eq!(image.pixels(), &[1, 2, 3, 4]);
    }
}
