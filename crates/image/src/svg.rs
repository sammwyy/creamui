use crate::{ImageData, ImageError};

/// How to size a rasterized SVG.
#[derive(Clone, Copy, Debug)]
pub enum SvgSize {
    /// The SVG's own `width`/`height` (or `viewBox`), rounded to pixels.
    Native,
    /// Fits within a `max` × `max` box, preserving aspect ratio — the usual
    /// choice for a square icon rendered at a specific pixel size.
    Max(u32),
    /// Exact pixel dimensions. Distorts the aspect ratio if it doesn't
    /// match the source.
    Exact(u32, u32),
}

impl ImageData {
    /// Decodes an SVG document, rasterizing it at `size`. Colors come from
    /// the document itself; use [`ImageData::tinted`] afterwards for a
    /// single-color icon that must match a theme.
    pub fn from_svg(bytes: &[u8], size: SvgSize) -> Result<Self, ImageError> {
        let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default())
            .map_err(|error| ImageError::Svg(error.to_string()))?;
        let source_size = tree.size();
        let (width, height) = match size {
            SvgSize::Native => (source_size.width(), source_size.height()),
            SvgSize::Max(max) => {
                let scale = max as f32 / source_size.width().max(source_size.height());
                (source_size.width() * scale, source_size.height() * scale)
            }
            SvgSize::Exact(width, height) => (width as f32, height as f32),
        };
        let width = (width.round() as u32).max(1);
        let height = (height.round() as u32).max(1);
        let transform = resvg::tiny_skia::Transform::from_scale(
            width as f32 / source_size.width(),
            height as f32 / source_size.height(),
        );
        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| ImageError::Svg("zero-sized pixmap".into()))?;
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        // `tiny_skia` stores premultiplied alpha; going through PNG (which
        // is straight-alpha) hands `from_rgba` the format it expects,
        // instead of it double-premultiplying an already-premultiplied
        // buffer.
        let png = pixmap
            .encode_png()
            .map_err(|error| ImageError::Svg(error.to_string()))?;
        Self::from_bytes(&png)
    }
}
