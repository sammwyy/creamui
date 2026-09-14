use crate::{Rect, Size};

/// Rects above this count merge to a single full-viewport rect, on the
/// assumption that redrawing that many separate regions costs more than
/// just redrawing everything.
pub const DEFAULT_MAX_RECTS: usize = 32;

/// Damaged area above this fraction of the viewport's area collapses to a
/// single full-viewport rect.
pub const DEFAULT_AREA_RATIO: f32 = 0.6;

/// Merges overlapping rects into their bounding union, then collapses the
/// result to one full-viewport rect once it exceeds `max_rects` or its
/// total area exceeds `area_ratio` of `viewport`'s area — whichever comes
/// first. Zero-or-negative-size rects are dropped. Order of the returned
/// rects is unspecified.
pub fn merge_damage(
    rects: &[Rect],
    viewport: Size,
    area_ratio: f32,
    max_rects: usize,
) -> Vec<Rect> {
    let mut merged: Vec<Rect> = rects
        .iter()
        .copied()
        .filter(|r| r.width > 0.0 && r.height > 0.0)
        .collect();

    let mut i = 0;
    while i < merged.len() {
        let mut j = i + 1;
        let mut absorbed = false;
        while j < merged.len() {
            if merged[i].intersect(merged[j]).is_some() {
                merged[i] = merged[i].union(merged[j]);
                merged.remove(j);
                absorbed = true;
            } else {
                j += 1;
            }
        }
        if !absorbed {
            i += 1;
        }
    }

    let viewport_area = (viewport.width.max(0.0) * viewport.height.max(0.0)) as f64;
    let damaged_area: f64 = merged
        .iter()
        .map(|r| r.width as f64 * r.height as f64)
        .sum();
    let over_threshold = merged.len() > max_rects
        || (viewport_area > 0.0 && damaged_area >= viewport_area * area_ratio as f64);

    if over_threshold {
        vec![Rect {
            x: 0.0,
            y: 0.0,
            width: viewport.width,
            height: viewport.height,
        }]
    } else {
        merged
    }
}

/// [`merge_damage`] with [`DEFAULT_AREA_RATIO`] and [`DEFAULT_MAX_RECTS`].
pub fn merge_damage_default(rects: &[Rect], viewport: Size) -> Vec<Rect> {
    merge_damage(rects, viewport, DEFAULT_AREA_RATIO, DEFAULT_MAX_RECTS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport() -> Size {
        Size {
            width: 1000.0,
            height: 1000.0,
        }
    }

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn disjoint_rects_stay_separate() {
        let rects = vec![rect(0.0, 0.0, 10.0, 10.0), rect(500.0, 500.0, 10.0, 10.0)];
        let merged = merge_damage_default(&rects, viewport());
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn overlapping_rects_merge_into_their_union() {
        let rects = vec![rect(0.0, 0.0, 20.0, 20.0), rect(10.0, 10.0, 20.0, 20.0)];
        let merged = merge_damage_default(&rects, viewport());
        assert_eq!(merged, vec![rect(0.0, 0.0, 30.0, 30.0)]);
    }

    #[test]
    fn a_chain_of_pairwise_overlaps_merges_transitively() {
        let rects = vec![
            rect(0.0, 0.0, 15.0, 10.0),
            rect(10.0, 0.0, 15.0, 10.0),
            rect(20.0, 0.0, 15.0, 10.0),
        ];
        let merged = merge_damage_default(&rects, viewport());
        assert_eq!(merged, vec![rect(0.0, 0.0, 35.0, 10.0)]);
    }

    #[test]
    fn zero_size_rects_are_dropped() {
        let rects = vec![rect(0.0, 0.0, 0.0, 10.0), rect(10.0, 10.0, 10.0, 10.0)];
        let merged = merge_damage_default(&rects, viewport());
        assert_eq!(merged, vec![rect(10.0, 10.0, 10.0, 10.0)]);
    }

    #[test]
    fn more_than_max_rects_collapses_to_full_viewport() {
        let rects: Vec<Rect> = (0..40)
            .map(|i| rect(i as f32 * 20.0, 0.0, 1.0, 1.0))
            .collect();
        let merged = merge_damage(&rects, viewport(), 1.0, 32);
        assert_eq!(merged, vec![rect(0.0, 0.0, 1000.0, 1000.0)]);
    }

    #[test]
    fn large_damaged_area_collapses_to_full_viewport() {
        let rects = vec![rect(0.0, 0.0, 900.0, 900.0)];
        let merged = merge_damage(&rects, viewport(), 0.5, 32);
        assert_eq!(merged, vec![rect(0.0, 0.0, 1000.0, 1000.0)]);
    }

    #[test]
    fn small_damage_under_every_threshold_is_returned_as_is() {
        let rects = vec![rect(0.0, 0.0, 10.0, 10.0)];
        let merged = merge_damage(&rects, viewport(), 0.5, 32);
        assert_eq!(merged, rects);
    }

    #[test]
    fn empty_input_is_empty_output() {
        assert!(merge_damage_default(&[], viewport()).is_empty());
    }

    #[test]
    fn zero_area_viewport_never_triggers_the_area_threshold() {
        let rects = vec![rect(0.0, 0.0, 10.0, 10.0)];
        let merged = merge_damage(
            &rects,
            Size {
                width: 0.0,
                height: 0.0,
            },
            0.1,
            32,
        );
        assert_eq!(merged, rects);
    }
}
