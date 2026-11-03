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
    aspect_ratio: Option<f32>,
}

impl GpuViewport {
    pub fn new(style: impl Into<creamui_core::Style>, slot: GpuFrameSlot) -> Self {
        Self {
            style: style.into(),
            slot,
            aspect_ratio: None,
        }
    }

    pub fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.aspect_ratio = (ratio.is_finite() && ratio > 0.0).then_some(ratio);
        self
    }
}

impl Widget for GpuViewport {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(frame) = self.slot.0.borrow().as_ref() {
            let bounds = if let Some(ratio) = self.aspect_ratio {
                let width = rect.width.min(rect.height * ratio);
                let height = rect.height.min(rect.width / ratio);
                Rect {
                    x: rect.x + (rect.width - width) / 2.0,
                    y: rect.y + (rect.height - height) / 2.0,
                    width,
                    height,
                }
            } else {
                rect
            };
            painter.draw_native_image(bounds, &frame.0);
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
        image_rect: Option<Rect>,
    }

    impl Painter for TestPainter {
        fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
        fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
        fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
        fn draw_native_image(&mut self, rect: Rect, image: &NativeImage) {
            self.image = Some(image.id());
            self.image_rect = Some(rect);
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
    fn native_viewport_letterboxes_to_project_aspect_ratio() {
        let slot = GpuFrameSlot::new();
        *slot.0.borrow_mut() = Some(GpuFrame(NativeImage::new(1u32)));
        let mut painter = TestPainter::default();
        GpuViewport::new(creamui_core::layout::Style::default(), slot)
            .aspect_ratio(16.0 / 9.0)
            .paint(
                &mut painter,
                Rect {
                    x: 10.0,
                    y: 20.0,
                    width: 200.0,
                    height: 200.0,
                },
            );
        let bounds = painter.image_rect.unwrap();
        assert!((bounds.x - 10.0).abs() < 0.01);
        assert!((bounds.y - 63.75).abs() < 0.01);
        assert!((bounds.width - 200.0).abs() < 0.01);
        assert!((bounds.height - 112.5).abs() < 0.01);
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
