use crate::FontFace;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::rc::Rc;
use swash::shape::ShapeContext;
use swash::text::cluster::{Boundary, CharCluster, CharInfo, Parser, Token, Whitespace};
use swash::text::{analyze, Codepoint, Script};

/// Line fitting tolerates this much overflow so that text measured at one
/// scale and laid out at another does not wrap on float rounding alone.
const WRAP_EPSILON: f32 = 0.001;

thread_local! {
    static SHAPER: RefCell<ShapeContext> = RefCell::new(ShapeContext::new());
    static LAYOUTS: RefCell<LayoutCache> = RefCell::new(LayoutCache::default());
}

const MAX_CACHED_LAYOUTS: usize = 2048;

#[derive(Default)]
struct LayoutCache {
    entries: HashMap<u64, CachedLayout>,
    tick: u64,
}

struct CachedLayout {
    face: u64,
    registry_generation: u64,
    text: Box<str>,
    px: u32,
    width: Option<u32>,
    height: Option<u32>,
    align: HorizontalAlign,
    layout: Rc<TextLayout>,
    used: u64,
}

/// Shares shaping between widget measurement and the renderer on this thread.
/// The cache is bounded independently of the renderer's rasterized glyph cache.
pub fn cached_layout(
    face: &FontFace,
    text: &str,
    px: f32,
    settings: &LayoutSettings,
) -> Rc<TextLayout> {
    let (px_bits, width_bits, height_bits) = (
        px.to_bits(),
        settings.max_width.map(f32::to_bits),
        settings.max_height.map(f32::to_bits),
    );
    let registry_generation = crate::registry_generation();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (
        face.id(),
        registry_generation,
        text,
        px_bits,
        width_bits,
        height_bits,
        settings.horizontal_align as u8,
    )
        .hash(&mut hasher);
    let key = hasher.finish();
    LAYOUTS.with(|layouts| {
        let mut cache = layouts.borrow_mut();
        cache.tick += 1;
        let tick = cache.tick;
        if let Some(entry) = cache.entries.get_mut(&key) {
            if entry.face == face.id()
                && entry.registry_generation == registry_generation
                && &*entry.text == text
                && entry.px == px_bits
                && entry.width == width_bits
                && entry.height == height_bits
                && entry.align == settings.horizontal_align
            {
                entry.used = tick;
                return entry.layout.clone();
            }
        }
        let shaped = Rc::new(layout(face, text, px, settings));
        if cache.entries.len() >= MAX_CACHED_LAYOUTS {
            let mut usage: Vec<u64> = cache.entries.values().map(|entry| entry.used).collect();
            let middle = usage.len() / 2;
            usage.select_nth_unstable(middle);
            let threshold = usage[middle];
            cache.entries.retain(|_, entry| entry.used > threshold);
        }
        cache.entries.insert(
            key,
            CachedLayout {
                face: face.id(),
                registry_generation,
                text: text.into(),
                px: px_bits,
                width: width_bits,
                height: height_bits,
                align: settings.horizontal_align,
                layout: shaped.clone(),
                used: tick,
            },
        );
        shaped
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HorizontalAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutSettings {
    /// Wraps lines longer than this and aligns them within it.
    pub max_width: Option<f32>,
    /// Centers the text block vertically within this height.
    pub max_height: Option<f32>,
    pub horizontal_align: HorizontalAlign,
}

/// A glyph to draw, with `x`/`y` its pen position on the baseline.
#[derive(Clone, Copy, Debug)]
pub struct PositionedGlyph {
    pub id: u16,
    /// Zero uses the requested face; other values index [`TextLayout::fallback_faces`].
    pub face: usize,
    pub x: f32,
    pub y: f32,
    pub byte_offset: usize,
}

/// One source character's caret position. Characters sharing a cluster
/// (ligatures, combining marks) split its advance evenly.
#[derive(Clone, Copy, Debug)]
pub struct CharPosition {
    pub byte_offset: usize,
    pub ch: char,
    pub x: f32,
    pub advance: f32,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub top: f32,
    pub baseline: f32,
    pub height: f32,
    pub width: f32,
    pub chars: Range<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct TextLayout {
    pub glyphs: Vec<PositionedGlyph>,
    pub fallback_faces: Vec<Rc<FontFace>>,
    pub chars: Vec<CharPosition>,
    pub lines: Vec<Line>,
    pub height: f32,
}

struct ShapedGlyph {
    id: u16,
    face: usize,
    x: f32,
    y: f32,
    advance: f32,
}

struct Cluster {
    source: Range<usize>,
    advance: f32,
    glyphs: Range<usize>,
    boundary: Boundary,
    whitespace: bool,
}

struct Shaped {
    clusters: Vec<Cluster>,
    glyphs: Vec<ShapedGlyph>,
    fallback_faces: Vec<Rc<FontFace>>,
}

/// Splits `text` into runs of one script each; common and inherited
/// characters (spaces, punctuation, marks) join the run around them.
fn script_runs(text: &str) -> Vec<(Range<usize>, Script)> {
    let mut runs: Vec<(Range<usize>, Script)> = Vec::new();
    for (offset, ch) in text.char_indices() {
        let end = offset + ch.len_utf8();
        let script = ch.script();
        let neutral = matches!(script, Script::Common | Script::Inherited | Script::Unknown);
        match runs.last_mut() {
            Some((range, run_script)) if neutral || *run_script == script => range.end = end,
            Some((range, run_script)) if matches!(run_script, Script::Common) => {
                range.end = end;
                *run_script = script;
            }
            _ => runs.push((offset..end, if neutral { Script::Common } else { script })),
        }
    }
    runs
}

fn shape(face: &FontFace, text: &str, px: f32) -> Shaped {
    let font = face.font_ref();
    let tokens: Vec<Token> = text
        .char_indices()
        .zip(analyze(text.chars()))
        .map(|((offset, ch), (properties, boundary))| Token {
            ch,
            offset: offset as u32,
            len: ch.len_utf8() as u8,
            info: CharInfo::new(properties, boundary),
            data: 0,
        })
        .collect();
    let mut shaped = Shaped {
        clusters: Vec::new(),
        glyphs: Vec::new(),
        fallback_faces: Vec::new(),
    };
    SHAPER.with(|context| {
        let mut context = context.borrow_mut();
        let mut cluster = CharCluster::new();
        for (range, script) in script_runs(text) {
            let first = tokens.partition_point(|token| (token.offset as usize) < range.start);
            let last = tokens.partition_point(|token| (token.offset as usize) < range.end);
            let mut parser = Parser::new(script, tokens[first..last].iter().copied());
            let mut prepared = Vec::new();
            while parser.next(&mut cluster) {
                let fallback = crate::fallback_face(face, &mut cluster);
                let face_index = fallback.map_or(0, |fallback| {
                    shaped
                        .fallback_faces
                        .iter()
                        .position(|candidate| candidate.id() == fallback.id())
                        .map_or_else(
                            || {
                                shaped.fallback_faces.push(fallback);
                                shaped.fallback_faces.len()
                            },
                            |index| index + 1,
                        )
                });
                prepared.push((cluster, face_index));
            }
            let mut start_index = 0;
            while start_index < prepared.len() {
                let face_index = prepared[start_index].1;
                let end_index = (start_index + 1..prepared.len())
                    .find(|&i| prepared[i].1 != face_index)
                    .unwrap_or(prepared.len());
                let selected = if face_index == 0 {
                    font
                } else {
                    shaped.fallback_faces[face_index - 1].font_ref()
                };
                let mut shaper = context.builder(selected).script(script).size(px).build();
                for (cluster, _) in &prepared[start_index..end_index] {
                    shaper.add_cluster(cluster);
                }
                shaper.shape_with(|glyph_cluster| {
                    let start = shaped.glyphs.len();
                    let control = glyph_cluster.info.whitespace() == Whitespace::Newline;
                    if !control {
                        shaped
                            .glyphs
                            .extend(glyph_cluster.glyphs.iter().map(|glyph| ShapedGlyph {
                                id: glyph.id,
                                face: face_index,
                                x: glyph.x,
                                y: glyph.y,
                                advance: glyph.advance,
                            }));
                    }
                    let source = glyph_cluster.source;
                    shaped.clusters.push(Cluster {
                        source: source.start as usize..source.end as usize,
                        advance: if control {
                            0.0
                        } else {
                            glyph_cluster.advance()
                        },
                        glyphs: start..shaped.glyphs.len(),
                        boundary: glyph_cluster.info.boundary(),
                        whitespace: glyph_cluster.info.is_whitespace(),
                    });
                });
                start_index = end_index;
            }
        }
    });
    shaped
}

/// Where each line starts, as cluster indices, breaking at mandatory breaks
/// and, past `max_width`, at the last line-break opportunity (or before the
/// overflowing cluster when the line has none).
fn break_lines(clusters: &[Cluster], max_width: f32) -> Vec<usize> {
    let mut starts = vec![0];
    let mut line_start = 0;
    let mut line_x = 0.0;
    let mut opportunity: Option<(usize, f32)> = None;
    let mut x = 0.0;
    for (index, cluster) in clusters.iter().enumerate() {
        if index > line_start {
            match cluster.boundary {
                Boundary::Mandatory => {
                    starts.push(index);
                    line_start = index;
                    line_x = x;
                    opportunity = None;
                }
                Boundary::Line => opportunity = Some((index, x)),
                _ => {}
            }
        }
        if !cluster.whitespace && x - line_x + cluster.advance > max_width + WRAP_EPSILON {
            let (start, start_x) = match opportunity {
                Some(found) if found.0 > line_start => found,
                _ if index > line_start => (index, x),
                _ => (line_start, line_x),
            };
            if start > line_start {
                starts.push(start);
                line_start = start;
                line_x = start_x;
                opportunity = None;
            }
        }
        x += cluster.advance;
    }
    starts
}

/// Lays `text` out with `face` at `px` pixels per em, top-left at the
/// origin.
pub fn layout(face: &FontFace, text: &str, px: f32, settings: &LayoutSettings) -> TextLayout {
    if text.is_empty() {
        return TextLayout::default();
    }
    let shaped = shape(face, text, px);
    let metrics = face.line_metrics(px);
    let starts = break_lines(
        &shaped.clusters,
        settings.max_width.unwrap_or(f32::INFINITY),
    );
    let height = metrics.line_height * starts.len() as f32;
    let top = match settings.max_height {
        Some(max_height) => ((max_height - height) * 0.5).floor(),
        None => 0.0,
    };
    let align = match settings.horizontal_align {
        HorizontalAlign::Left => 0.0,
        HorizontalAlign::Center => 0.5,
        HorizontalAlign::Right => 1.0,
    };

    let mut result = TextLayout {
        glyphs: Vec::with_capacity(shaped.glyphs.len()),
        fallback_faces: shaped.fallback_faces,
        chars: Vec::with_capacity(text.len()),
        lines: Vec::with_capacity(starts.len()),
        height,
    };
    for (line_index, &start) in starts.iter().enumerate() {
        let end = starts
            .get(line_index + 1)
            .copied()
            .unwrap_or(shaped.clusters.len());
        let clusters = &shaped.clusters[start..end];
        let width: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
        let x_offset = settings
            .max_width
            .map_or(0.0, |max_width| ((max_width - width) * align).floor());
        let line_top = top + metrics.line_height * line_index as f32;
        let baseline = line_top + metrics.ascent;
        let first_char = result.chars.len();
        let mut x = x_offset;
        for cluster in clusters {
            let mut pen = x;
            for glyph in &shaped.glyphs[cluster.glyphs.clone()] {
                result.glyphs.push(PositionedGlyph {
                    id: glyph.id,
                    face: glyph.face,
                    x: pen + glyph.x,
                    y: baseline - glyph.y,
                    byte_offset: cluster.source.start,
                });
                pen += glyph.advance;
            }
            let source = &text[cluster.source.clone()];
            let count = source.chars().count().max(1) as f32;
            let advance = cluster.advance / count;
            for (index, (offset, ch)) in source.char_indices().enumerate() {
                result.chars.push(CharPosition {
                    byte_offset: cluster.source.start + offset,
                    ch,
                    x: x + advance * index as f32,
                    advance,
                    line: line_index,
                });
            }
            x += cluster.advance;
        }
        result.lines.push(Line {
            top: line_top,
            baseline,
            height: metrics.line_height,
            width,
            chars: first_char..result.chars.len(),
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{resolve, FontWeight, DEFAULT_FAMILY};

    fn run(text: &str, settings: LayoutSettings) -> TextLayout {
        layout(
            &resolve(DEFAULT_FAMILY, FontWeight::Regular),
            text,
            16.0,
            &settings,
        )
    }

    fn line_texts<'a>(text: &'a str, layout: &TextLayout) -> Vec<&'a str> {
        layout
            .lines
            .iter()
            .map(|line| {
                let chars = &layout.chars[line.chars.clone()];
                let start = chars.first().map_or(0, |c| c.byte_offset);
                let end = chars.last().map_or(0, |c| c.byte_offset + c.ch.len_utf8());
                &text[start..end]
            })
            .collect()
    }

    #[test]
    fn cached_layout_reuses_shaping_for_equal_requests() {
        let face = resolve(DEFAULT_FAMILY, FontWeight::Regular);
        let left = LayoutSettings {
            max_width: Some(200.0),
            ..Default::default()
        };
        let first = cached_layout(&face, "shared measurement", 16.0, &left);
        let same = cached_layout(&face, "shared measurement", 16.0, &left);
        assert!(Rc::ptr_eq(&first, &same));
        let narrower = cached_layout(
            &face,
            "shared measurement",
            16.0,
            &LayoutSettings {
                max_width: Some(80.0),
                ..left
            },
        );
        assert!(!Rc::ptr_eq(&first, &narrower));
        let centered = cached_layout(
            &face,
            "shared measurement",
            16.0,
            &LayoutSettings {
                horizontal_align: HorizontalAlign::Center,
                ..left
            },
        );
        assert!(!Rc::ptr_eq(&first, &centered));
    }

    #[test]
    fn empty_text_has_no_lines() {
        let layout = run("", LayoutSettings::default());
        assert!(layout.lines.is_empty());
        assert_eq!(layout.height, 0.0);
    }

    #[test]
    fn every_character_gets_a_caret_position() {
        let text = "a b\u{e9}c";
        let layout = run(text, LayoutSettings::default());
        let offsets: Vec<usize> = layout.chars.iter().map(|c| c.byte_offset).collect();
        assert_eq!(offsets, vec![0, 1, 2, 3, 5]);
        assert!(layout.chars.windows(2).all(|pair| pair[1].x >= pair[0].x));
    }

    #[test]
    fn newlines_force_breaks() {
        let text = "one\ntwo\n\nthree";
        let layout = run(text, LayoutSettings::default());
        assert_eq!(
            line_texts(text, &layout),
            vec!["one\n", "two\n", "\n", "three"]
        );
        let line_height = layout.lines[0].height;
        assert_eq!(layout.height, line_height * 4.0);
        assert!(layout.lines[1].top > layout.lines[0].top);
    }

    #[test]
    fn wraps_at_word_boundaries_within_max_width() {
        let text = "alpha beta gamma delta";
        let natural = run(text, LayoutSettings::default()).lines[0].width;
        let layout = run(
            text,
            LayoutSettings {
                max_width: Some(natural * 0.6),
                ..LayoutSettings::default()
            },
        );
        assert_eq!(
            line_texts(text, &layout),
            vec!["alpha beta ", "gamma delta"]
        );
    }

    #[test]
    fn natural_width_fits_on_one_line() {
        let text = "Hello, CreamUI!";
        let natural = run(text, LayoutSettings::default()).lines[0].width;
        let layout = run(
            text,
            LayoutSettings {
                max_width: Some(natural),
                ..LayoutSettings::default()
            },
        );
        assert_eq!(layout.lines.len(), 1);
    }

    #[test]
    fn a_word_longer_than_the_line_is_split() {
        let layout = run(
            "abcdefghijklmnop",
            LayoutSettings {
                max_width: Some(30.0),
                ..LayoutSettings::default()
            },
        );
        assert!(layout.lines.len() > 1);
        assert!(layout
            .lines
            .iter()
            .all(|line| line.width <= 30.0 + WRAP_EPSILON));
    }

    #[test]
    fn alignment_and_vertical_centering_offset_the_block() {
        let left = run(
            "Hi",
            LayoutSettings {
                max_width: Some(200.0),
                max_height: Some(100.0),
                horizontal_align: HorizontalAlign::Left,
            },
        );
        let right = run(
            "Hi",
            LayoutSettings {
                max_width: Some(200.0),
                max_height: Some(100.0),
                horizontal_align: HorizontalAlign::Right,
            },
        );
        assert_eq!(left.chars[0].x, 0.0);
        let end = right.chars[1].x + right.chars[1].advance;
        assert!(end <= 200.0 && end > 199.0);
        assert!(left.lines[0].top > 0.0);
        assert_eq!(left.lines[0].top, ((100.0 - left.height) * 0.5).floor());
    }

    #[test]
    fn scripts_are_split_into_runs() {
        let runs = script_runs("ab 你好 cd");
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0], (0..3, Script::Latin));
        assert_eq!(runs[1].1, Script::Han);
    }

    #[test]
    fn missing_glyph_uses_an_installed_face_with_coverage() {
        let primary = resolve(DEFAULT_FAMILY, FontWeight::Regular);
        let primary_font = primary.font_ref();
        let primary_charmap = primary_font.charmap();
        let missing = ['\u{0905}', '\u{05d0}', '\u{4e2d}', '\u{1f600}']
            .into_iter()
            .find(|&ch| {
                primary_charmap.map(ch) == 0
                    && crate::system_font_index().iter().any(|(_, path)| {
                        FontFace::from_path(path)
                            .ok()
                            .is_some_and(|face| face.font_ref().charmap().map(ch) != 0)
                    })
            });
        let ch = missing.expect("an installed font must cover a script absent from the UI face");

        let text = format!("A{ch}B");
        let layout = layout(&primary, &text, 16.0, &LayoutSettings::default());
        let fallback = layout
            .glyphs
            .iter()
            .find(|glyph| glyph.byte_offset == 1)
            .expect("missing character should produce a glyph");
        assert!(fallback.face > 0);
        assert_ne!(fallback.id, 0);
        assert_ne!(layout.fallback_faces[fallback.face - 1].id(), primary.id());
    }
}
