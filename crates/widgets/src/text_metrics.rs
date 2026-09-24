//! Text measurement backing [`crate::raw::RawText`]'s `taffy` measure
//! function (see `creamui_core::Widget::measure`).
//!
//! Resolves faces through `creamui-fonts`'s registry and lays text out with
//! the same engine `creamui-render` paints with, without rasterizing.

use creamui_fonts::{FontFace, FontWeight, LayoutSettings, TextLayout, DEFAULT_FAMILY};
use std::rc::Rc;

fn font(family: Option<&str>) -> Rc<FontFace> {
    creamui_fonts::resolve(family.unwrap_or(DEFAULT_FAMILY), FontWeight::Regular)
}

fn wrapped(face: &FontFace, text: &str, font_size: f32, max_width: f32) -> TextLayout {
    creamui_fonts::layout(
        face,
        text,
        font_size,
        &LayoutSettings {
            max_width: Some(max_width),
            ..LayoutSettings::default()
        },
    )
}

/// A width large enough that single-line text never wraps against it, but
/// far from `f32::MAX` so alignment arithmetic on it stays finite.
const UNBOUNDED_WIDTH: f32 = 1_000_000.0;

/// Returns `(width, height)` in logical pixels for `text` set at
/// `font_size`, laid out as a single line within `max_width` (pass
/// [`UNBOUNDED_WIDTH`], exposed via [`unbounded_width`], for the text's
/// natural, unwrapped width).
///
/// The width is rounded up to whole pixels so a layout pass that snaps the
/// box to the pixel grid never makes the renderer wrap the text.
pub fn measure(text: &str, font_size: f32, max_width: f32) -> (f32, f32) {
    measure_weight(text, font_size, max_width, false)
}

pub fn measure_weight(text: &str, font_size: f32, max_width: f32, bold: bool) -> (f32, f32) {
    measure_family(text, font_size, max_width, None, bold)
}

/// Like [`measure_weight`], resolving `family` (a CSS-style stack) against
/// the font registry instead of the bundled default. `None` behaves exactly
/// like [`measure_weight`].
pub fn measure_family(
    text: &str,
    font_size: f32,
    max_width: f32,
    family: Option<&str>,
    bold: bool,
) -> (f32, f32) {
    let weight = if bold {
        FontWeight::Bold
    } else {
        FontWeight::Regular
    };
    let face = creamui_fonts::resolve(family.unwrap_or(DEFAULT_FAMILY), weight);
    let layout = wrapped(&face, text, font_size, max_width);
    let width = layout.lines.first().map_or(0.0, |line| line.width.ceil());
    // The wrapped height, not a single-line guess: at a narrow `max_width`
    // the text may wrap onto several lines that all need room.
    let height = layout.height.max(font_size * 1.4);
    (width.max(1.0), height)
}

/// See [`UNBOUNDED_WIDTH`].
pub fn unbounded_width() -> f32 {
    UNBOUNDED_WIDTH
}

/// Returns the closest UTF-8 insertion boundary for a horizontal point in a
/// single source line. Unlike repeatedly measuring every prefix, this builds
/// one font layout, which keeps pointer selection responsive on long lines.
pub fn byte_offset_at_x(text: &str, font_size: f32, x: f32) -> usize {
    byte_offset_at_x_family(text, font_size, x, None)
}

pub fn byte_offset_at_x_family(text: &str, font_size: f32, x: f32, family: Option<&str>) -> usize {
    let layout = wrapped(&font(family), text, font_size, UNBOUNDED_WIDTH);
    let mut offset = 0;
    for c in &layout.chars {
        if x < c.x + c.advance / 2.0 {
            return c.byte_offset;
        }
        offset = c.byte_offset + c.ch.len_utf8();
    }
    offset.min(text.len())
}

/// One glyph's position and advance width from a layout of a whole text
/// block, wrapped at `max_width` — the basis for word-wrap-aware
/// caret/selection/click math. `y`/`row_height` come from the row's own
/// metrics, not the glyph's own bounding box,
/// so every glyph on a row shares the same `y` regardless of ascender or
/// descender differences between characters (a "g" and an "A" sitting on
/// the same row must report the same row top).
pub struct LaidGlyph {
    pub byte_offset: usize,
    pub x: f32,
    pub y: f32,
    pub row_height: f32,
    pub advance: f32,
    pub ch: char,
}

/// Lays `text` out at `font_size`, wrapping at `max_width` and respecting
/// embedded `\n`s exactly as the renderer will (same font, same layout
/// engine), returning every character's position.
pub fn layout_family(
    text: &str,
    font_size: f32,
    max_width: f32,
    family: Option<&str>,
) -> Vec<LaidGlyph> {
    let layout = wrapped(&font(family), text, font_size, max_width);
    layout
        .chars
        .iter()
        .map(|c| {
            let line = &layout.lines[c.line];
            LaidGlyph {
                byte_offset: c.byte_offset,
                x: c.x,
                y: line.top,
                row_height: line.height,
                advance: c.advance,
                ch: c.ch,
            }
        })
        .collect()
}

/// The total height of `text` laid out the same way [`layout`] would —
/// pass this as a text block's height (instead of a taller container's
/// full height) so the renderer's own vertical centering has nothing to
/// center against and top-aligns instead, matching [`layout`]'s `y`s.
pub fn content_height_family(
    text: &str,
    font_size: f32,
    max_width: f32,
    family: Option<&str>,
) -> f32 {
    wrapped(&font(family), text, font_size, max_width).height
}

/// Wraps `text` at `max_width` exactly as the renderer will, and — if that
/// takes more than `max_lines` rows — trims it down to the longest prefix
/// whose rows (plus a trailing "…") still fit within `max_lines`. Returns
/// `text` unchanged when it already fits.
pub fn clamp_to_lines(
    text: &str,
    font_size: f32,
    max_width: f32,
    family: Option<&str>,
    max_lines: usize,
) -> String {
    if max_lines == 0 || text.is_empty() {
        return text.to_string();
    }
    let row_starts = |value: &str| -> Vec<usize> {
        let mut starts = Vec::new();
        let mut last_y = None;
        for glyph in layout_family(value, font_size, max_width, family) {
            if last_y != Some(glyph.y) {
                starts.push(glyph.byte_offset);
                last_y = Some(glyph.y);
            }
        }
        starts
    };
    let rows = row_starts(text);
    if rows.len() <= max_lines {
        return text.to_string();
    }
    let cut = rows.get(max_lines).copied().unwrap_or(text.len());
    let mut candidate = text[..cut.min(text.len())].trim_end().to_string();
    loop {
        let attempt = format!("{candidate}\u{2026}");
        if row_starts(&attempt).len() <= max_lines {
            return attempt;
        }
        if candidate.pop().is_none() {
            return attempt;
        }
        candidate = candidate.trim_end().to_string();
    }
}

/// The row height a lone, one-line layout at `font_size` gets — a fallback
/// for [`caret_xy`] and empty documents, where no glyph/row exists yet to
/// read a real one from.
pub fn row_height_family(font_size: f32, family: Option<&str>) -> f32 {
    content_height_family("A", font_size, UNBOUNDED_WIDTH, family)
}

/// Where a caret at `byte_offset` should be drawn within a `layout`ed
/// block — `(x, y, row_height)` — the start of the glyph at that offset, or
/// just past the previous glyph when `byte_offset` falls between glyphs
/// (end of a row, end of the text). `fallback_row_height` (see
/// [`row_height`]) is used only when `glyphs` is empty.
pub fn caret_xy(
    glyphs: &[LaidGlyph],
    byte_offset: usize,
    fallback_row_height: f32,
) -> (f32, f32, f32) {
    if let Some(g) = glyphs.iter().find(|g| g.byte_offset == byte_offset) {
        return (g.x, g.y, g.row_height);
    }
    if let Some(g) = glyphs.iter().rev().find(|g| g.byte_offset < byte_offset) {
        return (g.x + g.advance, g.y, g.row_height);
    }
    (0.0, 0.0, fallback_row_height)
}

/// The closest UTF-8 insertion boundary to point `(x, y)` in a `layout`ed,
/// possibly-wrapped block — the wrap-aware counterpart to
/// [`byte_offset_at_x`], picking a row by nearest `y` first.
pub fn byte_offset_at_point_family(
    text: &str,
    font_size: f32,
    max_width: f32,
    x: f32,
    y: f32,
    family: Option<&str>,
) -> usize {
    let glyphs = layout_family(text, font_size, max_width, family);
    let Some(row_y) = glyphs
        .iter()
        .map(|g| g.y)
        .min_by(|a, b| (a - y).abs().total_cmp(&(b - y).abs()))
    else {
        return 0;
    };
    let row: Vec<&LaidGlyph> = glyphs
        .iter()
        .filter(|g| (g.y - row_y).abs() < 0.5)
        .collect();
    for g in &row {
        if x < g.x + g.advance / 2.0 {
            return g.byte_offset;
        }
    }
    row.last()
        .map_or(0, |g| g.byte_offset + g.ch.len_utf8())
        .min(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longer_text_measures_wider() {
        let (short_width, _) = measure("Hi", 16.0, unbounded_width());
        let (long_width, _) = measure("Hello, CreamUI!", 16.0, unbounded_width());
        assert!(long_width > short_width);
    }

    #[test]
    fn constraining_max_width_does_not_exceed_it() {
        let (natural_width, _) = measure("Hello, CreamUI!", 16.0, unbounded_width());
        let (constrained_width, _) = measure("Hello, CreamUI!", 16.0, natural_width);
        assert!(
            (constrained_width - natural_width).abs() < 0.01,
            "measuring with max_width set to the natural width should reproduce that width with no wrap: got {constrained_width}, expected {natural_width}"
        );
    }

    #[test]
    fn text_that_already_fits_is_returned_unchanged() {
        let clamped = clamp_to_lines("Wireshark", 12.0, 90.0, None, 2);
        assert_eq!(clamped, "Wireshark");
    }

    #[test]
    fn text_needing_more_rows_than_allowed_gets_an_ellipsis() {
        let clamped = clamp_to_lines(
            "A very long application name that keeps going",
            12.0,
            90.0,
            None,
            2,
        );
        assert!(clamped.ends_with('\u{2026}'));
        assert!(clamped.len() < "A very long application name that keeps going".len());
    }

    #[test]
    fn clamped_text_fits_within_the_requested_rows() {
        let text = "A very long application name that keeps going and going";
        let clamped = clamp_to_lines(text, 12.0, 90.0, None, 2);
        let glyphs = layout_family(&clamped, 12.0, 90.0, None);
        let rows = glyphs
            .iter()
            .map(|glyph| glyph.y.to_bits())
            .collect::<std::collections::HashSet<_>>()
            .len();
        assert!(rows <= 2, "expected at most 2 rows, got {rows}");
    }
}
