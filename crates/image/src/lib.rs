//! Raster image support for CreamUI.
//!
//! Enable only the decoders an application uses: `png` (default), `jpeg`,
//! and/or `webp`.

mod background;
#[cfg(feature = "svg")]
mod svg;

use creamui_core::layout::Dimension;
use creamui_core::{Painter, Rect, Style, Styled, Widget};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

pub use background::{BackgroundImageLoader, LoadOutcome, ResourceId, ResourceReady};
#[cfg(feature = "svg")]
pub use svg::SvgSize;

/// How an [`Image`] fits its source pixels inside its layout box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageFit {
    /// Stretch to the layout box.
    Fill,
    /// Preserve aspect ratio; the complete image remains visible.
    Contain,
    /// Preserve aspect ratio while filling the layout box; excess is clipped.
    #[default]
    Cover,
    /// Keep the source pixel dimensions, anchored at the top-left.
    None,
}

/// A decoded RGBA image ready for reuse across widget-tree rebuilds.
#[derive(Clone)]
pub struct ImageData {
    width: u32,
    height: u32,
    pixels: Arc<[u8]>,
}

impl ImageData {
    /// Decodes PNG, JPEG, or WebP bytes when its corresponding crate feature
    /// is enabled.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ImageError> {
        let image = image_rs::load_from_memory(bytes).map_err(ImageError::Decode)?;
        let rgba = image.to_rgba8();
        Self::from_rgba(rgba.width(), rgba.height(), rgba.into_raw())
    }

    /// Reads and decodes an image from the local filesystem.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ImageError> {
        let bytes = std::fs::read(path).map_err(ImageError::Io)?;
        Self::from_bytes(&bytes)
    }

    /// Creates image data from straight-alpha RGBA8 pixels.
    pub fn from_rgba(width: u32, height: u32, mut pixels: Vec<u8>) -> Result<Self, ImageError> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4));
        if width == 0 || height == 0 || expected != Some(pixels.len()) {
            return Err(ImageError::InvalidPixels {
                width,
                height,
                length: pixels.len(),
            });
        }
        for pixel in pixels.chunks_exact_mut(4) {
            let alpha = pixel[3] as u16;
            pixel[0] = (pixel[0] as u16 * alpha / 255) as u8;
            pixel[1] = (pixel[1] as u16 * alpha / 255) as u8;
            pixel[2] = (pixel[2] as u16 * alpha / 255) as u8;
        }
        Ok(Self {
            width,
            height,
            pixels: pixels.into(),
        })
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

    /// Recolors every visible pixel to `color`, keeping each pixel's own
    /// alpha (its coverage/opacity) — turns the image into a solid-color
    /// silhouette. Works on any `ImageData`, decoded SVG or raster alike,
    /// which is what makes a single icon file reusable across a light and a
    /// dark theme: decode once, then tint to whatever the active theme's
    /// icon color is.
    pub fn tinted(mut self, color: creamui_theme::Color) -> Self {
        let pixels = Arc::make_mut(&mut self.pixels);
        for pixel in pixels.chunks_exact_mut(4) {
            let alpha = pixel[3] as u16;
            pixel[0] = (color.r as u16 * alpha / 255) as u8;
            pixel[1] = (color.g as u16 * alpha / 255) as u8;
            pixel[2] = (color.b as u16 * alpha / 255) as u8;
        }
        self
    }
}

/// Errors returned while loading or validating [`ImageData`].
#[derive(Debug)]
pub enum ImageError {
    Io(std::io::Error),
    Decode(image_rs::ImageError),
    InvalidPixels {
        width: u32,
        height: u32,
        length: usize,
    },
    Svg(String),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "could not read image: {error}"),
            Self::Decode(error) => write!(f, "could not decode image: {error}"),
            Self::InvalidPixels {
                width,
                height,
                length,
            } => write!(
                f,
                "expected {} RGBA bytes for {width}×{height}, got {length}",
                *width as usize * *height as usize * 4
            ),
            Self::Svg(error) => write!(f, "could not render SVG: {error}"),
        }
    }
}
impl std::error::Error for ImageError {}

/// A layoutable image widget backed by reusable decoded [`ImageData`].
pub struct Image {
    data: ImageData,
    style: Style,
    fit: ImageFit,
}

impl Image {
    pub fn new(data: ImageData) -> Self {
        Self {
            style: creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(data.width as f32),
                    height: Dimension::Length(data.height as f32),
                },
                ..Default::default()
            }
            .into(),
            data,
            fit: ImageFit::Cover,
        }
    }

    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }
    pub fn data(&self) -> &ImageData {
        &self.data
    }

    fn destination(&self, rect: Rect) -> Rect {
        let source_width = self.data.width as f32;
        let source_height = self.data.height as f32;
        let scale = match self.fit {
            ImageFit::Fill => return rect,
            ImageFit::Contain => (rect.width / source_width).min(rect.height / source_height),
            ImageFit::Cover => (rect.width / source_width).max(rect.height / source_height),
            ImageFit::None => 1.0,
        };
        let width = source_width * scale;
        let height = source_height * scale;
        Rect {
            x: rect.x + (rect.width - width) / 2.0,
            y: rect.y + (rect.height - height) / 2.0,
            width,
            height,
        }
    }
}

impl Widget for Image {
    fn style(&self) -> creamui_core::Style {
        self.style.clone()
    }

    /// Identity-based, not content-based: the same decoded `ImageData`
    /// (e.g. cloned out of a cache) always yields the same pointer, and a
    /// fresh decode of identical pixels gets a fresh `Arc` allocation and a
    /// different one. Collides only if an `Arc<[u8]>` is freed and a new
    /// allocation happens to reuse its exact address before this fingerprint
    /// is compared against — not ruled out by the type system, but not a
    /// realistic concern for how `ImageData` is actually produced/cached.
    fn paint_fingerprint(&self) -> Option<u64> {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (Arc::as_ptr(&self.data.pixels) as *const u8 as usize).hash(&mut hasher);
        self.data.width.hash(&mut hasher);
        self.data.height.hash(&mut hasher);
        self.fit.hash(&mut hasher);
        self.style
            .paint
            .corner_radius
            .map(f32::to_bits)
            .hash(&mut hasher);
        Some(hasher.finish())
    }

    // `ImageFit::Contain` letterboxes rather than covering `rect`, and draws
    // no background of its own, so it can leave real gaps the same way text
    // does.
    fn paints_transparently(&self) -> bool {
        true
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let corner_radius = self.style.paint.corner_radius.unwrap_or(0.0);
        if matches!(self.fit, ImageFit::Cover) || corner_radius > 0.0 {
            painter.push_clip_rounded(rect, corner_radius);
            painter.draw_rgba_image(
                self.destination(rect),
                self.data.pixels(),
                self.data.width,
                self.data.height,
            );
            painter.pop_clip();
        } else {
            painter.draw_rgba_image(
                self.destination(rect),
                self.data.pixels(),
                self.data.width,
                self.data.height,
            );
        }
    }
}

impl Styled for Image {
    fn set_style(&mut self, style: Style) {
        self.style = style;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creamui_core::{render_frame, Size, TextAlign};

    #[derive(Default)]
    struct PainterSpy {
        image: Option<(Rect, u32, u32)>,
    }
    impl Painter for PainterSpy {
        fn fill_rect(&mut self, _: Rect, _: creamui_theme::Color, _: f32) {}
        fn stroke_rect(&mut self, _: Rect, _: creamui_theme::Color, _: f32, _: f32) {}
        fn fill_text(&mut self, _: Rect, _: &str, _: creamui_theme::Color, _: f32, _: TextAlign) {}
        fn draw_rgba_image(&mut self, rect: Rect, _: &[u8], width: u32, height: u32) {
            self.image = Some((rect, width, height));
        }
    }

    #[test]
    fn contain_preserves_the_source_aspect_ratio() {
        let data = ImageData::from_rgba(4, 2, vec![255; 32]).unwrap();
        let image = Image::new(data)
            .layout(creamui_core::layout::Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(100.),
                    height: Dimension::Length(100.),
                },
                ..Default::default()
            })
            .fit(ImageFit::Contain);
        let mut painter = PainterSpy::default();
        render_frame(
            Box::new(image),
            Size {
                width: 100.,
                height: 100.,
            },
            &mut painter,
        );
        let (rect, width, height) = painter.image.unwrap();
        assert_eq!((width, height), (4, 2));
        assert_eq!((rect.width, rect.height), (100., 50.));
    }

    #[test]
    fn rejects_image_sizes_that_overflow_rgba_buffer_length() {
        let result = ImageData::from_rgba(u32::MAX, u32::MAX, Vec::new());
        assert!(matches!(result, Err(ImageError::InvalidPixels { .. })));
    }

    #[test]
    fn fingerprint_is_stable_across_clones_of_the_same_decoded_data() {
        let data = ImageData::from_rgba(2, 2, vec![255; 16]).unwrap();
        let a = Image::new(data.clone());
        let b = Image::new(data);
        assert_eq!(a.paint_fingerprint(), b.paint_fingerprint());
    }

    #[test]
    fn fingerprint_differs_for_independently_decoded_identical_pixels() {
        let a = Image::new(ImageData::from_rgba(2, 2, vec![255; 16]).unwrap());
        let b = Image::new(ImageData::from_rgba(2, 2, vec![255; 16]).unwrap());
        assert_ne!(a.paint_fingerprint(), b.paint_fingerprint());
    }

    #[test]
    fn tinted_recolors_visible_pixels_but_keeps_their_alpha() {
        // A half-transparent red pixel, straight alpha in, premultiplied out.
        let data = ImageData::from_rgba(1, 1, vec![255, 0, 0, 128])
            .unwrap()
            .tinted(creamui_theme::Color::rgb(0, 255, 0));
        let [r, g, b, a] = data.pixels() else {
            unreachable!()
        };
        assert_eq!(*r, 0);
        assert!(*g > 0, "green channel should carry the tint");
        assert_eq!(*b, 0);
        assert_eq!(*a, 128, "alpha must survive the tint unchanged");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn from_svg_rasterizes_and_tints() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
            <rect x="0" y="0" width="10" height="10" fill="#123456"/>
        </svg>"##;
        let data = ImageData::from_svg(svg, SvgSize::Max(8)).unwrap();
        assert_eq!(data.width().max(data.height()), 8);
        let tinted = data.tinted(creamui_theme::Color::rgb(255, 255, 255));
        let last = tinted.pixels().len() - 4;
        // The center of a fully opaque fill should end up fully white, not
        // the source document's navy blue.
        assert_eq!(&tinted.pixels()[last - 4..last], &[255, 255, 255, 255]);
    }
}
