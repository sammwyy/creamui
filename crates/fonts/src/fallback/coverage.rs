use std::mem::size_of;
use swash::text::cluster::{CharCluster, Status};

use crate::FontFace;

pub(super) struct Coverage(Vec<(u32, u32)>);

impl Coverage {
    pub(super) fn new(face: &FontFace) -> Self {
        let font = face.font_ref();
        let charmap = font.charmap();
        let mut points = Vec::new();
        charmap.enumerate(|codepoint, glyph| {
            if glyph != 0 && codepoint <= 0x10ffff {
                points.push(codepoint);
            }
        });
        // Symbol charmaps can alias low codepoints without enumerating those aliases.
        for codepoint in 0..=255_u32 {
            if charmap.map(codepoint) != 0 {
                points.push(codepoint);
            }
        }
        points.sort_unstable();
        points.dedup();
        let mut ranges: Vec<(u32, u32)> = Vec::new();
        for point in points {
            if let Some((_, end)) = ranges.last_mut().filter(|(_, end)| *end + 1 == point) {
                *end = point;
            } else {
                ranges.push((point, point));
            }
        }
        Self(ranges)
    }

    pub(super) fn contains(&self, ch: char) -> bool {
        let codepoint = ch as u32;
        let index = self.0.partition_point(|(_, end)| *end < codepoint);
        self.0
            .get(index)
            .is_some_and(|(start, _)| *start <= codepoint)
    }

    pub(super) fn improves(&self, cluster: &CharCluster) -> bool {
        let mut probe = *cluster;
        probe.map(|ch| u16::from(self.contains(ch))) != Status::Discard
    }

    pub(super) fn bytes(&self) -> usize {
        self.0.capacity() * size_of::<(u32, u32)>()
    }
}
