//! Sketch entities — geometric primitives in 2D.

use glam::DVec2;
use serde::{Serialize, Deserialize};

/// Entity ID (index into the sketch's entity list).
pub type EntityId = usize;

/// A point ID — references a specific point within an entity.
/// (entity_index, point_index) where point_index selects which point:
/// For Line: 0=start, 1=end. For Circle: 0=center. For Arc: 0=center, 1=start, 2=end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PointRef {
    pub entity: EntityId,
    pub point: usize,
}

impl PointRef {
    pub fn new(entity: EntityId, point: usize) -> Self {
        Self { entity, point }
    }
}

/// A 2D sketch entity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SketchEntity {
    /// A free point.
    Point { pos: DVec2 },
    /// A line segment.
    Line { start: DVec2, end: DVec2 },
    /// A circle.
    Circle { center: DVec2, radius: f64 },
    /// A circular arc.
    Arc { center: DVec2, start: DVec2, end: DVec2, radius: f64 },
}

impl SketchEntity {
    pub fn point(x: f64, y: f64) -> Self {
        Self::Point { pos: DVec2::new(x, y) }
    }

    pub fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Self::Line {
            start: DVec2::new(x0, y0),
            end: DVec2::new(x1, y1),
        }
    }

    pub fn circle(cx: f64, cy: f64, r: f64) -> Self {
        Self::Circle {
            center: DVec2::new(cx, cy),
            radius: r,
        }
    }

    /// Get the parameter vector for this entity (used by the solver).
    pub fn params(&self) -> Vec<f64> {
        match self {
            Self::Point { pos } => vec![pos.x, pos.y],
            Self::Line { start, end } => vec![start.x, start.y, end.x, end.y],
            Self::Circle { center, radius } => vec![center.x, center.y, *radius],
            Self::Arc { center, start, end, radius } => {
                vec![center.x, center.y, start.x, start.y, end.x, end.y, *radius]
            }
        }
    }

    /// Set parameters from a flat vector (inverse of params()).
    pub fn set_params(&mut self, p: &[f64]) {
        match self {
            Self::Point { pos } => {
                pos.x = p[0]; pos.y = p[1];
            }
            Self::Line { start, end } => {
                start.x = p[0]; start.y = p[1];
                end.x = p[2]; end.y = p[3];
            }
            Self::Circle { center, radius } => {
                center.x = p[0]; center.y = p[1];
                *radius = p[2];
            }
            Self::Arc { center, start, end, radius } => {
                center.x = p[0]; center.y = p[1];
                start.x = p[2]; start.y = p[3];
                end.x = p[4]; end.y = p[5];
                *radius = p[6];
            }
        }
    }

    /// Number of scalar parameters.
    pub fn param_count(&self) -> usize {
        match self {
            Self::Point { .. } => 2,
            Self::Line { .. } => 4,
            Self::Circle { .. } => 3,
            Self::Arc { .. } => 7,
        }
    }

    /// Get a specific point by index.
    pub fn get_point(&self, idx: usize) -> DVec2 {
        match self {
            Self::Point { pos } => *pos,
            Self::Line { start, end } => {
                if idx == 0 { *start } else { *end }
            }
            Self::Circle { center, .. } => *center,
            Self::Arc { center, start, end, .. } => {
                match idx {
                    0 => *center,
                    1 => *start,
                    _ => *end,
                }
            }
        }
    }

    /// Length of entity.
    pub fn length(&self) -> f64 {
        match self {
            Self::Point { .. } => 0.0,
            Self::Line { start, end } => (*end - *start).length(),
            Self::Circle { radius, .. } => std::f64::consts::TAU * radius,
            Self::Arc { radius, start, end, center } => {
                let a1 = (*start - *center).to_angle();
                let a2 = (*end - *center).to_angle();
                let mut da = a2 - a1;
                if da < 0.0 { da += std::f64::consts::TAU; }
                radius * da
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_params() {
        let e = SketchEntity::point(3.0, 4.0);
        assert_eq!(e.param_count(), 2);
        assert_eq!(e.params(), vec![3.0, 4.0]);
    }

    #[test]
    fn line_params_roundtrip() {
        let mut e = SketchEntity::line(0.0, 0.0, 10.0, 5.0);
        let _p = e.params();
        e.set_params(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(e.params(), vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn line_length() {
        let e = SketchEntity::line(0.0, 0.0, 3.0, 4.0);
        assert!((e.length() - 5.0).abs() < 1e-10);
    }
}
