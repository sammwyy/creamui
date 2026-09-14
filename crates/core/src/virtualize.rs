use std::ops::Range;

fn highest_power_of_two_leq(n: usize) -> usize {
    if n == 0 {
        0
    } else {
        1 << (usize::BITS - 1 - n.leading_zeros())
    }
}

/// A Fenwick tree over per-item heights: point updates and prefix-sum
/// queries in `O(log n)`, so a scroll offset maps to an item index without
/// summing every preceding item's height.
pub struct HeightIndex {
    heights: Vec<f32>,
    tree: Vec<f32>,
}

impl HeightIndex {
    pub fn new(heights: &[f32]) -> Self {
        let mut index = HeightIndex {
            heights: heights.to_vec(),
            tree: vec![0.0; heights.len() + 1],
        };
        for (i, &height) in heights.iter().enumerate() {
            index.add(i, height);
        }
        index
    }

    pub fn uniform(count: usize, height: f32) -> Self {
        HeightIndex::new(&vec![height; count])
    }

    pub fn len(&self) -> usize {
        self.heights.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heights.is_empty()
    }

    pub fn height(&self, index: usize) -> f32 {
        self.heights[index]
    }

    pub fn set_height(&mut self, index: usize, height: f32) {
        let delta = height - self.heights[index];
        if delta == 0.0 {
            return;
        }
        self.heights[index] = height;
        self.add(index, delta);
    }

    pub fn push(&mut self, height: f32) {
        self.heights.push(height);
        self.tree.push(0.0);
        self.add(self.heights.len() - 1, height);
    }

    pub fn truncate(&mut self, len: usize) {
        if len >= self.heights.len() {
            return;
        }
        self.heights.truncate(len);
        self.tree = vec![0.0; len + 1];
        for i in 0..len {
            let height = self.heights[i];
            self.add(i, height);
        }
    }

    /// Sum of every height before `index`.
    pub fn offset(&self, index: usize) -> f32 {
        self.prefix_sum(index)
    }

    pub fn total_height(&self) -> f32 {
        self.prefix_sum(self.heights.len())
    }

    /// The index whose span `[offset(index), offset(index) + height(index))`
    /// contains `at`, clamped to the last item when `at` is beyond
    /// `total_height()`.
    pub fn index_at_offset(&self, at: f32) -> usize {
        if self.heights.is_empty() {
            return 0;
        }
        let mut pos = 0usize;
        let mut remaining = at.max(0.0);
        let mut step = highest_power_of_two_leq(self.heights.len());
        while step > 0 {
            let next = pos + step;
            if next <= self.heights.len() && self.tree[next] <= remaining {
                pos = next;
                remaining -= self.tree[next];
            }
            step >>= 1;
        }
        pos.min(self.heights.len() - 1)
    }

    fn add(&mut self, index: usize, delta: f32) {
        let mut i = index + 1;
        while i < self.tree.len() {
            self.tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    fn prefix_sum(&self, count: usize) -> f32 {
        let mut i = count;
        let mut sum = 0.0;
        while i > 0 {
            sum += self.tree[i];
            i -= i & i.wrapping_neg();
        }
        sum
    }
}

/// The item range `index` should mount for `scroll_offset`/`viewport_height`,
/// padded by `overscan` items on each side and clamped to `index`'s bounds.
pub fn visible_range(
    index: &HeightIndex,
    scroll_offset: f32,
    viewport_height: f32,
    overscan: usize,
) -> Range<usize> {
    if index.is_empty() {
        return 0..0;
    }
    let start = index.index_at_offset(scroll_offset);
    let end = index
        .index_at_offset(scroll_offset + viewport_height)
        .saturating_add(1)
        .min(index.len());
    let start = start.saturating_sub(overscan);
    let end = (end + overscan).min(index.len());
    start..end.max(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_height_sums_every_item() {
        let index = HeightIndex::uniform(10, 20.0);
        assert_eq!(index.total_height(), 200.0);
    }

    #[test]
    fn offset_is_the_prefix_sum_before_an_item() {
        let index = HeightIndex::new(&[10.0, 20.0, 30.0]);
        assert_eq!(index.offset(0), 0.0);
        assert_eq!(index.offset(1), 10.0);
        assert_eq!(index.offset(2), 30.0);
    }

    #[test]
    fn set_height_updates_offsets_of_later_items_only() {
        let mut index = HeightIndex::new(&[10.0, 20.0, 30.0]);
        index.set_height(0, 50.0);
        assert_eq!(index.offset(0), 0.0);
        assert_eq!(index.offset(1), 50.0);
        assert_eq!(index.offset(2), 70.0);
        assert_eq!(index.total_height(), 100.0);
    }

    #[test]
    fn index_at_offset_finds_the_item_spanning_a_point() {
        let index = HeightIndex::new(&[10.0, 20.0, 30.0]);
        assert_eq!(index.index_at_offset(0.0), 0);
        assert_eq!(index.index_at_offset(9.9), 0);
        assert_eq!(index.index_at_offset(10.0), 1);
        assert_eq!(index.index_at_offset(29.9), 1);
        assert_eq!(index.index_at_offset(30.0), 2);
        assert_eq!(index.index_at_offset(1000.0), 2);
    }

    #[test]
    fn push_extends_total_height_and_stays_queryable() {
        let mut index = HeightIndex::new(&[10.0, 20.0]);
        index.push(5.0);
        assert_eq!(index.len(), 3);
        assert_eq!(index.total_height(), 35.0);
        assert_eq!(index.offset(2), 30.0);
    }

    #[test]
    fn truncate_shrinks_and_drops_trailing_height() {
        let mut index = HeightIndex::new(&[10.0, 20.0, 30.0]);
        index.truncate(1);
        assert_eq!(index.len(), 1);
        assert_eq!(index.total_height(), 10.0);
    }

    #[test]
    fn visible_range_covers_the_viewport_with_overscan() {
        let index = HeightIndex::uniform(100, 10.0);
        let range = visible_range(&index, 100.0, 50.0, 2);
        assert_eq!(range, 8..18);
    }

    #[test]
    fn visible_range_clamps_overscan_at_the_start_and_end() {
        let index = HeightIndex::uniform(10, 10.0);
        let range = visible_range(&index, 0.0, 30.0, 5);
        assert_eq!(range, 0..9);

        let range = visible_range(&index, 70.0, 30.0, 5);
        assert_eq!(range, 2..10);
    }

    #[test]
    fn visible_range_on_an_empty_index_is_empty() {
        let index = HeightIndex::new(&[]);
        assert_eq!(visible_range(&index, 0.0, 100.0, 2), 0..0);
    }
}
