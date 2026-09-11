//! CPU presentation: blits the CPU-rasterized RGBA buffer straight to the
//! window surface via `softbuffer`, with no GPU instance/adapter/device
//! involved.

use creamui_platform::Window;
use std::num::NonZeroU32;
use std::sync::Arc;

pub struct CpuState {
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    width: u32,
    height: u32,
}

impl CpuState {
    pub fn new(window: Arc<Window>) -> Self {
        let context =
            softbuffer::Context::new(window.clone()).expect("failed to create softbuffer context");
        let surface = softbuffer::Surface::new(&context, window)
            .expect("failed to create softbuffer surface");
        CpuState {
            surface,
            width: 0,
            height: 0,
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
    pub fn present(&mut self, rgba: &[u8], width: u32, height: u32) {
        if width != self.width || height != self.height {
            self.resize(width, height);
        }

        let mut buffer = self
            .surface
            .buffer_mut()
            .expect("failed to acquire softbuffer buffer");
        // softbuffer's pixel format is `0RGB` packed into a native-endian
        // u32; alpha is dropped since the window itself (not this blit) is
        // what controls transparency, via `WindowOptions::transparent`.
        for (px, chunk) in buffer.iter_mut().zip(rgba.chunks_exact(4)) {
            let [r, g, b, _a] = [chunk[0], chunk[1], chunk[2], chunk[3]];
            *px = (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b);
        }
        buffer
            .present()
            .expect("failed to present softbuffer buffer");
    }
}
