//! Software presentation of the CPU-rasterized frame, uploading only the
//! damaged regions.
//!
//! Wayland goes through [`wayland_shm`], which uses an `Argb8888` buffer so
//! transparent windows keep per-pixel alpha. Other platforms use
//! `softbuffer`, fed premultiplied `0xAARRGGBB` pixels (X11 honors the alpha
//! byte on 32-bit visuals).

#[cfg(target_os = "linux")]
mod wayland_shm;

use crate::display_list::Bounds;
use creamui_platform::PlatformWindow;
use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
use std::num::NonZeroU32;
use std::sync::Arc;
use tiny_skia::Pixmap;

pub enum SoftwareSurface {
    #[cfg(target_os = "linux")]
    Wayland(wayland_shm::ShmSurface),
    Softbuffer(SoftbufferSurface),
}

impl SoftwareSurface {
    pub fn new(window: Arc<dyn PlatformWindow>) -> Result<Self, String> {
        let is_wayland = matches!(
            window.display_handle().map(|h| h.as_raw()),
            Ok(RawDisplayHandle::Wayland(_))
        );
        #[cfg(target_os = "linux")]
        if is_wayland {
            return wayland_shm::ShmSurface::new(window).map(SoftwareSurface::Wayland);
        }
        let _ = is_wayland;
        SoftbufferSurface::new(window).map(SoftwareSurface::Softbuffer)
    }

    /// Presents `frame`, copying only `regions` when the platform buffer
    /// already holds the rest of it.
    pub fn present(&mut self, frame: &Pixmap, regions: &[Bounds]) {
        match self {
            #[cfg(target_os = "linux")]
            SoftwareSurface::Wayland(surface) => surface.present(frame, regions),
            SoftwareSurface::Softbuffer(surface) => surface.present(frame, regions),
        }
    }
}

/// Converts premultiplied RGBA bytes into native-endian `0xAARRGGBB`.
pub(crate) fn argb(rgba: &[u8]) -> u32 {
    u32::from_be_bytes([rgba[3], rgba[0], rgba[1], rgba[2]])
}

pub(crate) fn span(region: &Bounds, width: u32, height: u32) -> (usize, usize, usize, usize) {
    let clamp = |v: f32, max: u32| (v.max(0.0) as u32).min(max) as usize;
    (
        clamp(region.x0, width),
        clamp(region.y0, height),
        clamp(region.x1, width),
        clamp(region.y1, height),
    )
}

/// Tracks which frame regions each swapchain buffer is missing.
pub(crate) struct BufferHistory {
    previous: Vec<Bounds>,
}

impl BufferHistory {
    pub fn new() -> Self {
        BufferHistory {
            previous: Vec::new(),
        }
    }

    /// Regions to copy into a buffer last written `age` presents ago, or
    /// `None` when the whole frame must be copied.
    pub fn regions_for_age(&mut self, age: u8, current: &[Bounds]) -> Option<Vec<Bounds>> {
        let regions = match age {
            1 => Some(current.to_vec()),
            2 => Some(current.iter().chain(&self.previous).copied().collect()),
            _ => None,
        };
        self.previous = current.to_vec();
        regions
    }
}

pub struct SoftbufferSurface {
    surface: softbuffer::Surface<Arc<dyn PlatformWindow>, Arc<dyn PlatformWindow>>,
    width: u32,
    height: u32,
    history: BufferHistory,
}

impl SoftbufferSurface {
    fn new(window: Arc<dyn PlatformWindow>) -> Result<Self, String> {
        let context = softbuffer::Context::new(window.clone()).map_err(|e| e.to_string())?;
        let surface = softbuffer::Surface::new(&context, window).map_err(|e| e.to_string())?;
        Ok(SoftbufferSurface {
            surface,
            width: 0,
            height: 0,
            history: BufferHistory::new(),
        })
    }

    fn present(&mut self, frame: &Pixmap, regions: &[Bounds]) {
        let (width, height) = (frame.width(), frame.height());
        if (width, height) != (self.width, self.height) {
            if let Err(err) = self.surface.resize(
                NonZeroU32::new(width).expect("pixmap width is non-zero"),
                NonZeroU32::new(height).expect("pixmap height is non-zero"),
            ) {
                log::error!("creamui-render: softbuffer resize failed: {err}");
                return;
            }
            self.width = width;
            self.height = height;
        }
        let mut buffer = match self.surface.buffer_mut() {
            Ok(buffer) => buffer,
            Err(err) => {
                log::error!("creamui-render: softbuffer buffer unavailable: {err}");
                return;
            }
        };
        let full = [Bounds::new(0.0, 0.0, width as f32, height as f32)];
        let copy = self
            .history
            .regions_for_age(buffer.age(), regions)
            .unwrap_or_else(|| full.to_vec());
        let data = frame.data();
        for region in &copy {
            let (x0, y0, x1, y1) = span(region, width, height);
            for y in y0..y1 {
                let row = y * width as usize;
                for (dst, src) in buffer[row + x0..row + x1]
                    .iter_mut()
                    .zip(data[(row + x0) * 4..(row + x1) * 4].chunks_exact(4))
                {
                    *dst = argb(src);
                }
            }
        }
        let damage: Vec<softbuffer::Rect> = regions
            .iter()
            .filter_map(|region| {
                let (x0, y0, x1, y1) = span(region, width, height);
                Some(softbuffer::Rect {
                    x: x0 as u32,
                    y: y0 as u32,
                    width: NonZeroU32::new((x1 - x0) as u32)?,
                    height: NonZeroU32::new((y1 - y0) as u32)?,
                })
            })
            .collect();
        if let Err(err) = buffer.present_with_damage(&damage) {
            log::error!("creamui-render: softbuffer present failed: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_keeps_alpha() {
        assert_eq!(argb(&[0x11, 0x22, 0x33, 0x80]), 0x8011_2233);
    }

    #[test]
    fn buffer_age_selects_the_regions_to_copy() {
        let a = [Bounds::new(0.0, 0.0, 1.0, 1.0)];
        let b = [Bounds::new(5.0, 5.0, 6.0, 6.0)];
        let mut history = BufferHistory::new();
        assert_eq!(history.regions_for_age(0, &a), None);
        assert_eq!(history.regions_for_age(2, &b), Some(vec![b[0], a[0]]));
        assert_eq!(history.regions_for_age(1, &a), Some(vec![a[0]]));
        assert_eq!(history.regions_for_age(3, &a), None);
    }

    #[test]
    fn spans_are_clamped_to_the_frame() {
        let region = Bounds::new(-4.0, 2.0, 50.0, 8.0);
        assert_eq!(span(&region, 20, 5), (0, 2, 20, 5));
    }
}
