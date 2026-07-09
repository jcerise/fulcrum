//! Shared math types beyond what glam provides.

use glam::Vec2;

/// An axis-aligned rectangle stored as min/max corners.
///
/// Which corner is "min" depends on the space: in world space (+Y up) `min` is bottom-left; in
/// image/pixel space (+Y down) `min` is top-left. The math is identical either way.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    /// Corner with the smaller coordinates.
    pub min: Vec2,
    /// Corner with the larger coordinates.
    pub max: Vec2,
}

impl Rect {
    /// Rectangle from two corners (caller guarantees `min <= max` per axis).
    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// Rectangle from a center point and a size.
    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        Self {
            min: center - size / 2.0,
            max: center + size / 2.0,
        }
    }

    /// Rectangle from its min corner and a size.
    pub fn from_min_size(min: Vec2, size: Vec2) -> Self {
        Self {
            min,
            max: min + size,
        }
    }

    /// Width and height.
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    /// Center point.
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) / 2.0
    }

    /// Is `point` inside (edges inclusive)?
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Do the rectangles overlap? Touching edges count as overlapping.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::vec2;

    #[test]
    fn contains_is_edge_inclusive() {
        let r = Rect::new(vec2(0.0, 0.0), vec2(10.0, 5.0));
        assert!(r.contains(vec2(5.0, 2.5)));
        assert!(r.contains(vec2(0.0, 0.0)), "min corner");
        assert!(r.contains(vec2(10.0, 5.0)), "max corner");
        assert!(!r.contains(vec2(10.1, 2.0)));
    }

    #[test]
    fn overlaps_including_touching_edges() {
        let a = Rect::new(vec2(0.0, 0.0), vec2(10.0, 10.0));
        assert!(
            a.overlaps(&Rect::new(vec2(5.0, 5.0), vec2(15.0, 15.0))),
            "partial"
        );
        assert!(
            a.overlaps(&Rect::new(vec2(2.0, 2.0), vec2(3.0, 3.0))),
            "contained"
        );
        assert!(
            a.overlaps(&Rect::new(vec2(10.0, 0.0), vec2(20.0, 10.0))),
            "touching edge"
        );
        assert!(
            a.overlaps(&Rect::new(vec2(10.0, 10.0), vec2(20.0, 20.0))),
            "touching corner"
        );
        assert!(
            !a.overlaps(&Rect::new(vec2(10.5, 0.0), vec2(20.0, 10.0))),
            "separated"
        );
    }

    #[test]
    fn constructors_agree() {
        let a = Rect::from_center_size(vec2(5.0, 5.0), vec2(10.0, 10.0));
        let b = Rect::from_min_size(vec2(0.0, 0.0), vec2(10.0, 10.0));
        assert_eq!(a, b);
        assert_eq!(a.center(), vec2(5.0, 5.0));
        assert_eq!(a.size(), vec2(10.0, 10.0));
    }
}
