use crate::FontError;
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;
use swash::{CacheKey, FontRef};

enum FontData {
    Mapped(Mmap),
    Owned(Box<[u8]>),
}

impl FontData {
    fn bytes(&self) -> &[u8] {
        match self {
            FontData::Mapped(map) => map,
            FontData::Owned(bytes) => bytes,
        }
    }
}

/// A parsed font. Tables and glyph outlines are read from the underlying
/// bytes on demand, so only the pages actually touched become resident.
pub struct FontFace {
    data: FontData,
    offset: u32,
    key: CacheKey,
}

/// Vertical metrics of one line of text, in pixels, rounded up to whole
/// pixels so baselines land on the pixel grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
    pub line_height: f32,
}

impl FontFace {
    fn new(data: FontData) -> Result<Self, FontError> {
        let font = FontRef::from_index(data.bytes(), 0)
            .ok_or_else(|| FontError("unsupported or invalid font data".into()))?;
        let (offset, key) = (font.offset, font.key);
        Ok(FontFace { data, offset, key })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FontError> {
        Self::new(FontData::Owned(bytes.into()))
    }

    pub fn from_path(path: &Path) -> Result<Self, FontError> {
        let file = File::open(path).map_err(|e| FontError(format!("{}: {e}", path.display())))?;
        // SAFETY: font files are treated as read-only assets; truncating one
        // while the process runs is outside what CreamUI supports.
        let map = unsafe { Mmap::map(&file) }
            .map_err(|e| FontError(format!("{}: {e}", path.display())))?;
        Self::new(FontData::Mapped(map))
    }

    pub fn font_ref(&self) -> FontRef<'_> {
        FontRef {
            data: self.data.bytes(),
            offset: self.offset,
            key: self.key,
        }
    }

    /// Identifies this face for as long as it is alive.
    pub fn id(&self) -> u64 {
        self.key.value()
    }

    pub fn glyph_count(&self) -> u16 {
        self.font_ref().metrics(&[]).glyph_count
    }

    pub fn line_metrics(&self, px: f32) -> LineMetrics {
        let metrics = self.font_ref().metrics(&[]).scale(px);
        LineMetrics {
            ascent: metrics.ascent.ceil(),
            descent: metrics.descent.ceil(),
            line_gap: metrics.leading.ceil(),
            line_height: (metrics.ascent + metrics.descent + metrics.leading).ceil(),
        }
    }
}
