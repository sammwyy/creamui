use crate::WindowHandle;
use creamui_core::{NativeImage, Painter, Rect, Widget};
use creamui_theme::Color;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub struct GpuFrame(NativeImage);

impl GpuFrame {
    pub fn new(view: wgpu::TextureView) -> Self {
        Self(NativeImage::new(view))
    }
}

#[derive(Clone, Default)]
pub struct GpuFrameSlot(Rc<RefCell<Option<GpuFrame>>>);

impl GpuFrameSlot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(&self, frame: GpuFrame, window: &WindowHandle) {
        *self.0.borrow_mut() = Some(frame);
        window.request_repaint();
    }

    pub fn clear(&self, window: &WindowHandle) {
        self.0.borrow_mut().take();
        window.request_repaint();
    }
}

pub struct GpuViewport {
    pub style: creamui_core::Style,
    slot: GpuFrameSlot,
}

impl GpuViewport {
    pub fn new(style: impl Into<creamui_core::Style>, slot: GpuFrameSlot) -> Self {
        Self {
            style: style.into(),
            slot,
        }
    }
}

impl Widget for GpuViewport {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(frame) = self.slot.0.borrow().as_ref() {
            painter.draw_native_image(rect, &frame.0);
        } else {
            painter.fill_rect(rect, Color::rgb(32, 32, 32), 0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::{Renderer, Size, TextAlign};

    #[derive(Default)]
    struct TestPainter {
        image: Option<u64>,
    }

    impl Painter for TestPainter {
        fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
        fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
        fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
        fn draw_native_image(&mut self, _: Rect, image: &NativeImage) {
            self.image = Some(image.id());
        }
    }

    #[test]
    fn repaint_reads_only_the_latest_published_frame() {
        let slot = GpuFrameSlot::new();
        let first = GpuFrame(NativeImage::new(1u32));
        let last = GpuFrame(NativeImage::new(2u32));
        *slot.0.borrow_mut() = Some(first);
        *slot.0.borrow_mut() = Some(last.clone());
        let size = Size {
            width: 50.0,
            height: 50.0,
        };
        let mut renderer = Renderer::new();
        let mut painter = TestPainter::default();
        renderer.render(
            Box::new(GpuViewport::new(
                creamui_core::layout::Style::default(),
                slot,
            )),
            size,
            &mut painter,
        );
        assert_eq!(painter.image, Some(last.0.id()));
    }

    #[test]
    fn software_renderer_draws_placeholder_for_gpu_only_frames() {
        let slot = GpuFrameSlot::new();
        *slot.0.borrow_mut() = Some(GpuFrame(NativeImage::new(1u32)));
        let mut renderer = Renderer::new();
        let mut recorder = crate::SceneRecorder::new();
        recorder.begin(
            16,
            16,
            1.0,
            Color::rgb(0, 0, 0),
            creamui_theme::ColorScheme::default(),
        );
        renderer.render(
            Box::new(GpuViewport::new(
                creamui_core::layout::Style {
                    size: creamui_core::layout::Size {
                        width: creamui_core::layout::Dimension::Length(16.0),
                        height: creamui_core::layout::Dimension::Length(16.0),
                    },
                    ..Default::default()
                },
                slot,
            )),
            Size {
                width: 16.0,
                height: 16.0,
            },
            &mut recorder,
        );
        let list = recorder.finish();
        let mut raster = crate::Rasterizer::new(16, 16);
        raster.render(&list, &crate::Damage::Full);
        assert_eq!(
            &raster.pixmap().data()[(8 * 16 + 8) * 4..(8 * 16 + 8) * 4 + 4],
            &[32, 32, 32, 255]
        );
    }
}
