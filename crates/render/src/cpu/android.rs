use crate::display_list::Bounds;
use std::mem::MaybeUninit;
use tiny_skia::Pixmap;

#[cfg(target_os = "android")]
use creamui_platform::PlatformWindow;
#[cfg(target_os = "android")]
use ndk::{hardware_buffer_format::HardwareBufferFormat, native_window::NativeWindow};
#[cfg(target_os = "android")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(target_os = "android")]
use std::sync::Arc;

#[cfg(target_os = "android")]
pub struct AndroidSurface {
    native: NativeWindow,
    _window: Arc<dyn PlatformWindow>,
    size: (u32, u32),
}

#[cfg(target_os = "android")]
impl AndroidSurface {
    pub fn new(window: Arc<dyn PlatformWindow>) -> Result<Self, String> {
        let RawWindowHandle::AndroidNdk(handle) = window
            .window_handle()
            .map_err(|err| err.to_string())?
            .as_raw()
        else {
            return Err("expected an Android native window".to_owned());
        };
        // The platform window keeps the handle valid while its native reference is acquired.
        let native = unsafe { NativeWindow::clone_from_ptr(handle.a_native_window.cast()) };
        Ok(Self {
            native,
            _window: window,
            size: (0, 0),
        })
    }

    pub fn present(&mut self, frame: &Pixmap, regions: &[Bounds]) -> bool {
        let size = (frame.width(), frame.height());
        let resized = self.size != size;
        if resized {
            let (Ok(width), Ok(height)) = (i32::try_from(size.0), i32::try_from(size.1)) else {
                log::error!("creamui-render: Android buffer size is out of range: {size:?}");
                return false;
            };
            if let Err(err) = self.native.set_buffers_geometry(
                width,
                height,
                Some(HardwareBufferFormat::R8G8B8A8_UNORM),
            ) {
                log::error!("creamui-render: Android buffer resize failed: {err}");
                return false;
            }
            self.size = size;
        }
        let bounds = if resized {
            Bounds::new(0.0, 0.0, size.0 as f32, size.1 as f32)
        } else {
            damage_bounds(regions, size.0, size.1)
        };
        let mut dirty = ndk::native_window::Rect {
            left: bounds.x0 as i32,
            top: bounds.y0 as i32,
            right: bounds.x1 as i32,
            bottom: bounds.y1 as i32,
        };
        let mut buffer = match self.native.lock(Some(&mut dirty)) {
            Ok(buffer) => buffer,
            Err(err) => {
                log::warn!("creamui-render: Android buffer lock failed: {err}");
                return false;
            }
        };
        if buffer.width() != size.0 as usize
            || buffer.height() != size.1 as usize
            || buffer.format() != HardwareBufferFormat::R8G8B8A8_UNORM
        {
            log::error!("creamui-render: Android returned incompatible buffer geometry");
            return false;
        }
        let stride = buffer.stride() * 4;
        let bounds = Bounds::new(
            dirty.left as f32,
            dirty.top as f32,
            dirty.right as f32,
            dirty.bottom as f32,
        );
        copy_rgba(
            frame,
            buffer.bytes().expect("RGBA buffer is mappable"),
            stride,
            bounds,
        );
        true
    }
}

fn damage_bounds(regions: &[Bounds], width: u32, height: u32) -> Bounds {
    regions
        .iter()
        .fold(
            Bounds::new(width as f32, height as f32, 0.0, 0.0),
            |a, b| {
                Bounds::new(
                    a.x0.min(b.x0),
                    a.y0.min(b.y0),
                    a.x1.max(b.x1),
                    a.y1.max(b.y1),
                )
            },
        )
        .round_out()
        .intersect(Bounds::new(0.0, 0.0, width as f32, height as f32))
}

fn copy_rgba(frame: &Pixmap, output: &mut [MaybeUninit<u8>], stride: usize, bounds: Bounds) {
    let (x0, y0, x1, y1) = super::span(&bounds, frame.width(), frame.height());
    let x0 = x0 * 4;
    let x1 = x1 * 4;
    let source_stride = frame.width() as usize * 4;
    for y in y0..y1 {
        let source = &frame.data()[y * source_stride + x0..y * source_stride + x1];
        let destination = &mut output[y * stride + x0..y * stride + x1];
        for (destination, source) in destination.iter_mut().zip(source) {
            destination.write(*source);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_damage_copies_expanded_regions_without_touching_stride_padding() {
        let mut frame = Pixmap::new(3, 3).unwrap();
        frame.fill(tiny_skia::Color::from_rgba8(80, 160, 240, 128));
        let mut output = vec![MaybeUninit::new(0xff); 16 * 3];
        copy_rgba(&frame, &mut output, 16, Bounds::new(-1.0, 1.0, 5.0, 8.0));
        // Every byte starts initialized, including the untouched rows and padding.
        let output: Vec<u8> = output
            .into_iter()
            .map(|byte| unsafe { byte.assume_init() })
            .collect();
        assert_eq!(&output[..16], &[0xff; 16]);
        for y in 1..3 {
            assert_eq!(
                &output[y * 16..y * 16 + 12],
                &frame.data()[y * 12..(y + 1) * 12]
            );
            assert_eq!(&output[y * 16 + 12..(y + 1) * 16], &[0xff; 4]);
        }
    }

    #[test]
    fn disjoint_damage_requests_one_clamped_rectangle() {
        let bounds = damage_bounds(
            &[
                Bounds::new(-4.0, 2.0, 4.0, 5.0),
                Bounds::new(7.0, 3.0, 20.0, 8.0),
            ],
            10,
            6,
        );
        assert_eq!(bounds, Bounds::new(0.0, 2.0, 10.0, 6.0));
        assert_eq!(damage_bounds(&[], 10, 6), Bounds::EMPTY);
        assert_eq!(
            damage_bounds(&[Bounds::new(2.5, 1.5, 3.2, 4.2)], 10, 6),
            Bounds::new(2.0, 1.0, 4.0, 5.0)
        );
    }

    #[test]
    fn partial_copy_preserves_other_pixels_and_alpha() {
        let mut frame = Pixmap::new(3, 2).unwrap();
        frame.fill(tiny_skia::Color::from_rgba8(80, 160, 240, 128));
        let mut output = vec![MaybeUninit::new(0xff); 32];
        copy_rgba(&frame, &mut output, 16, Bounds::new(1.0, 0.0, 2.0, 1.0));
        // Every byte starts initialized, including the untouched pixels.
        let output: Vec<u8> = output
            .into_iter()
            .map(|byte| unsafe { byte.assume_init() })
            .collect();
        assert_eq!(&output[4..8], &[40, 80, 120, 128]);
        assert_eq!(&output[..4], &[0xff; 4]);
        assert_eq!(&output[8..], &[0xff; 24]);
    }
}
