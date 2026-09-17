//! CPU presentation: blits the CPU-rasterized RGBA buffer straight to the
//! window surface via `softbuffer`, with no GPU instance/adapter/device
//! involved.

use creamui_platform::PlatformWindow;
use std::num::NonZeroU32;
use std::sync::Arc;

pub struct CpuState {
    surface: softbuffer::Surface<Arc<dyn PlatformWindow>, Arc<dyn PlatformWindow>>,
    width: u32,
    height: u32,
    transparent: bool,
}

impl CpuState {
    pub fn new(window: Arc<dyn PlatformWindow>, transparent: bool) -> Self {
        let context =
            softbuffer::Context::new(window.clone()).expect("failed to create softbuffer context");
        let surface = softbuffer::Surface::new(&context, window)
            .expect("failed to create softbuffer surface");
        CpuState {
            surface,
            width: 0,
            height: 0,
            transparent,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        self.surface
            .resize(
                NonZeroU32::new(width).unwrap(),
                NonZeroU32::new(height).unwrap(),
            )
            .expect("failed to resize softbuffer surface");
        self.width = width;
        self.height = height;
    }

    /// Uploads `rgba` (straight RGBA8, `width * height * 4` bytes) and
    /// presents it to the window surface.
    ///
    /// `softbuffer`'s Wayland backend always allocates an alpha-less
    /// `Xrgb8888` buffer, so a transparent window can't be blended
    /// per-pixel here the way the GPU backend does — a `0RGB` write over a
    /// fully transparent frame would show up as solid black, hiding
    /// whatever is behind it. A `transparent` window whose frame is
    /// entirely empty (alpha 0 everywhere, e.g. an idle overlay with
    /// nothing to show) skips presenting instead, so the surface is never
    /// mapped/committed and stays truly invisible. A transparent window
    /// with any opaque content still blits opaque `0RGB` as before — CPU
    /// backend just can't make part of that frame see-through.
    pub fn present(&mut self, rgba: &[u8], width: u32, height: u32) {
        if self.transparent && rgba.chunks_exact(4).all(|chunk| chunk[3] == 0) {
            return;
        }
        if width != self.width || height != self.height {
            self.resize(width, height);
        }

        let mut buffer = self
            .surface
            .buffer_mut()
            .expect("failed to acquire softbuffer buffer");
        for (px, chunk) in buffer.iter_mut().zip(rgba.chunks_exact(4)) {
            let [r, g, b, _a] = [chunk[0], chunk[1], chunk[2], chunk[3]];
            *px = (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b);
        }
        buffer
            .present()
            .expect("failed to present softbuffer buffer");
    }
}
