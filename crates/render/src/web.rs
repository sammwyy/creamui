//! Web presentation: copies the rasterized frame into the platform canvas.

use creamui_platform::PlatformWindow;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

pub struct WebState {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
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
        Self { canvas, context }
    }

    pub fn present(&mut self, rgba: &[u8], width: u32, height: u32) {
        // The `<canvas>` backing pixel buffer (its `width`/`height` IDL
        // attributes) is independent of its CSS box size and the platform never
        // touches it — left at the browser default (300x150) it silently
        // crops every frame and the CSS-sized box then stretches that crop,
        // rendering blurry. Keep it in lockstep with the painted frame so a
        // CSS-fullscreen canvas gets a full-resolution, uncropped frame.
        if self.canvas.width() != width || self.canvas.height() != height {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
        }
        let data =
            ImageData::new_with_u8_clamped_array_and_sh(wasm_bindgen::Clamped(rgba), width, height)
                .expect("could not create browser image data");
        self.context
            .put_image_data(&data, 0.0, 0.0)
            .expect("could not present CreamUI frame to canvas");
    }
}
