//! Web presentation: copies the damaged parts of the rasterized frame into
//! the platform canvas.

use crate::display_list::Bounds;
use creamui_platform::PlatformWindow;
use std::sync::Arc;
use tiny_skia::Pixmap;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

pub struct WebState {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    straight: Vec<u8>,
}

impl WebState {
    pub fn new(window: Arc<dyn PlatformWindow>) -> Self {
        let canvas = window
            .canvas()
            .expect("the platform did not create a canvas for the web demo");
        let context = canvas
            .get_context("2d")
            .expect("could not obtain canvas context")
            .expect("browser does not support a 2D canvas context")
            .dyn_into::<CanvasRenderingContext2d>()
            .expect("canvas context was not CanvasRenderingContext2d");
        Self {
            canvas,
            context,
            straight: Vec::new(),
        }
    }

    pub fn present(&mut self, frame: &Pixmap, regions: &[Bounds]) {
        let (width, height) = (frame.width(), frame.height());
        // The canvas backing store is independent of its CSS box; left at the
        // browser default it would crop and then stretch every frame.
        let resized = self.canvas.width() != width || self.canvas.height() != height;
        if resized {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
        }
        self.straight.clear();
        self.straight.extend(frame.pixels().iter().flat_map(|px| {
            let c = px.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        }));
        let data = match ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&self.straight),
            width,
            height,
        ) {
            Ok(data) => data,
            Err(err) => {
                log::error!("creamui-render: could not create canvas image data: {err:?}");
                return;
            }
        };
        let full = [Bounds::new(0.0, 0.0, width as f32, height as f32)];
        for region in if resized { &full[..] } else { regions } {
            let result = self
                .context
                .put_image_data_with_dirty_x_and_dirty_y_and_dirty_width_and_dirty_height(
                    &data,
                    0.0,
                    0.0,
                    region.x0 as f64,
                    region.y0 as f64,
                    region.width() as f64,
                    region.height() as f64,
                );
            if let Err(err) = result {
                log::error!("creamui-render: could not present frame to canvas: {err:?}");
            }
        }
    }
}
