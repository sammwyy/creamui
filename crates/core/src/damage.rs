use crate::{Rect, Size};

/// Rects above this count are coalesced, cheapest union first, until at
/// most this many remain, on the assumption that redrawing that many
/// separate regions costs more than redrawing a few slightly larger ones.
pub const DEFAULT_MAX_RECTS: usize = 32;

/// Damaged area above this fraction of the viewport's area collapses to a
/// single full-viewport rect.
pub const DEFAULT_AREA_RATIO: f32 = 0.6;

/// How many rects [`coalesce`] compares pairwise; beyond it, rects are
/// first bucketed into a coarse grid.
const PAIRWISE_LIMIT: usize = 32;

/// Merges overlapping rects into their bounding union and coalesces the
/// result down to at most `max_rects`, preferring the unions that add the
/// least undamaged area. Collapses to one full-viewport rect once the
/// total area exceeds `area_ratio` of `viewport`'s area.
/// Zero-or-negative-size rects are dropped. Order of the returned rects is
/// unspecified.
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
    if merged.len() <= PAIRWISE_LIMIT {
        absorb_overlaps(&mut merged);
    }
    if merged.len() > max_rects {
        coalesce(&mut merged, max_rects.max(1));
    }

    let viewport_area = (viewport.width.max(0.0) * viewport.height.max(0.0)) as f64;
    let damaged_area: f64 = merged.iter().map(|r| area(*r)).sum();
    if viewport_area > 0.0 && damaged_area >= viewport_area * area_ratio as f64 {
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

fn area(r: Rect) -> f64 {
    r.width as f64 * r.height as f64
}

/// Replaces every group of transitively overlapping rects with its union.
fn absorb_overlaps(rects: &mut Vec<Rect>) {
    let mut i = 0;
    while i < rects.len() {
        let mut j = i + 1;
        let mut absorbed = false;
        while j < rects.len() {
            if rects[i].intersect(rects[j]).is_some() {
                rects[i] = rects[i].union(rects[j]);
                rects.remove(j);
                absorbed = true;
            } else {
                j += 1;
            }
        }
        if !absorbed {
            i += 1;
        }
    }
}

/// Unions the rects whose centers share a cell of a coarse grid over
/// their bounding box, leaving at most `GRID * GRID`.
fn bucket(rects: &mut Vec<Rect>) {
    const GRID: usize = 5;
    let bounds = rects.iter().skip(1).fold(rects[0], |a, b| a.union(*b));
    let mut cells: [Option<Rect>; GRID * GRID] = [None; GRID * GRID];
    for r in rects.iter() {
        let cell = |center: f32, start: f32, length: f32| {
            (((center - start) / length * GRID as f32) as usize).min(GRID - 1)
        };
        let column = cell(r.x + r.width / 2.0, bounds.x, bounds.width);
        let row = cell(r.y + r.height / 2.0, bounds.y, bounds.height);
        let slot = &mut cells[row * GRID + column];
        *slot = Some(slot.map_or(*r, |merged| merged.union(*r)));
    }
    *rects = cells.into_iter().flatten().collect();
}

/// Merges rects until at most `max_rects` remain, always taking the pair
/// whose union covers the least area the pair didn't already cover.
fn coalesce(rects: &mut Vec<Rect>, max_rects: usize) {
    if rects.len() > PAIRWISE_LIMIT.max(max_rects) {
        bucket(rects);
    }
    absorb_overlaps(rects);
    while rects.len() > max_rects {
        let mut best = (f64::INFINITY, 0, 1);
        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                let waste = area(rects[i].union(rects[j])) - area(rects[i]) - area(rects[j]);
                if waste < best.0 {
                    best = (waste, i, j);
                }
            }
        }
        let (_, i, j) = best;
        rects[i] = rects[i].union(rects[j]);
        rects.swap_remove(j);
        absorb_overlaps(rects);
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
    fn more_than_max_rects_coalesce_into_nearby_unions() {
        let column = |x: f32| (0..20).map(move |i| rect(x, i as f32 * 30.0, 10.0, 20.0));
        let rects: Vec<Rect> = column(0.0).chain(column(900.0)).collect();
        let merged = merge_damage(&rects, viewport(), 0.5, 8);
        assert!(merged.len() <= 8);
        let covered: f64 = merged.iter().map(|r| area(*r)).sum();
        assert!(covered <= 2.0 * 10.0 * 590.0, "{merged:?}");
        for r in &rects {
            assert!(merged.iter().any(|m| m.union(*r) == *m), "{r:?} lost");
        }
    }

    #[test]
    fn many_tiny_rects_coalesce_without_losing_any() {
        let rects: Vec<Rect> = (0..500)
            .map(|i| rect((i * 37 % 990) as f32, (i * 53 % 990) as f32, 2.0, 2.0))
            .collect();
        let merged = merge_damage(&rects, viewport(), 1.0, 8);
        assert!(merged.len() <= 8);
        for r in &rects {
            assert!(merged.iter().any(|m| m.union(*r) == *m), "{r:?} lost");
        }
    }

    #[test]
    fn coalesced_damage_over_the_area_ratio_collapses_to_full_viewport() {
        let rects = vec![rect(0.0, 0.0, 10.0, 10.0), rect(990.0, 990.0, 10.0, 10.0)];
        let merged = merge_damage(&rects, viewport(), 0.5, 1);
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
