/// A 2D point in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// A width/height pair in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// An axis-aligned rectangle in logical pixels, positioned by its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    /// The overlapping region of two rects, or `None` if they don't overlap.
    /// Used to clip a widget's own interactive/painted area to whatever
    /// portion of it its ancestors (e.g. a scroll view) actually show.
    pub fn intersect(&self, other: Rect) -> Option<Rect> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);
        if x2 > x1 && y2 > y1 {
            Some(Rect {
                x: x1,
                y: y1,
                width: x2 - x1,
                height: y2 - y1,
            })
        } else {
            None
        }
    }

    /// Whether the closed rects share at least one point, so zero-sized rects
    /// on or inside `other` still count.
    pub fn overlaps(&self, other: Rect) -> bool {
        self.x <= other.x + other.width
            && other.x <= self.x + self.width
            && self.y <= other.y + other.height
            && other.y <= self.y + self.height
    }

    /// `self` grown by `amount` on every side.
    pub fn inflate(&self, amount: f32) -> Rect {
        Rect {
            x: self.x - amount,
            y: self.y - amount,
            width: self.width + amount * 2.0,
            height: self.height + amount * 2.0,
        }
    }

    /// The smallest rect containing both `self` and `other`.
    pub fn union(&self, other: Rect) -> Rect {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.width).max(other.x + other.width);
        let y2 = (self.y + self.height).max(other.y + other.height);
        Rect {
            x: x1,
            y: y1,
            width: x2 - x1,
            height: y2 - y1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_inside_point() {
        let r = Rect {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
        };
        assert!(r.contains(Point { x: 15.0, y: 15.0 }));
    }

    #[test]
    fn rect_excludes_outside_point() {
        let r = Rect {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
        };
        assert!(!r.contains(Point { x: 100.0, y: 100.0 }));
    }

    #[test]
    fn rect_boundary_is_inclusive() {
        let r = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        assert!(r.contains(Point { x: 10.0, y: 10.0 }));
        assert!(r.contains(Point { x: 0.0, y: 0.0 }));
    }

    #[test]
    fn intersect_overlapping_rects() {
        let a = Rect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        };
        let b = Rect {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
        };
        assert_eq!(
            a.intersect(b),
            Some(Rect {
                x: 10.0,
                y: 10.0,
                width: 10.0,
                height: 10.0
            })
        );
    }

    #[test]
    fn intersect_non_overlapping_rects_is_none() {
        let a = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let b = Rect {
            x: 100.0,
            y: 100.0,
            width: 10.0,
            height: 10.0,
        };
        assert_eq!(a.intersect(b), None);
    }

    #[test]
    fn union_bounds_two_disjoint_rects() {
        let a = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let b = Rect {
            x: 50.0,
            y: 20.0,
            width: 10.0,
            height: 10.0,
        };
        assert_eq!(
            a.union(b),
            Rect {
                x: 0.0,
                y: 0.0,
                width: 60.0,
                height: 30.0,
            }
        );
    }

    #[test]
    fn union_of_a_rect_with_itself_is_unchanged() {
        let a = Rect {
            x: 5.0,
            y: 5.0,
            width: 10.0,
            height: 10.0,
        };
        assert_eq!(a.union(a), a);
    }
}
