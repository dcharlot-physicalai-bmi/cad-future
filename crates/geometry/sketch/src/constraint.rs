//! Sketch constraints — geometric relationships between entities.

use crate::entity::{EntityId, PointRef};
use serde::{Serialize, Deserialize};

/// Constraint ID.
pub type ConstraintId = usize;

/// A geometric constraint between sketch entities.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Constraint {
    /// Fix a point at a specific location.
    Fixed { point: PointRef, x: f64, y: f64 },
    /// Two points coincide.
    Coincident { a: PointRef, b: PointRef },
    /// A point lies on a horizontal line (y = constant).
    Horizontal { a: PointRef, b: PointRef },
    /// A point lies on a vertical line (x = constant).
    Vertical { a: PointRef, b: PointRef },
    /// Distance between two points.
    Distance { a: PointRef, b: PointRef, value: f64 },
    /// A line has a specific length.
    LineLength { entity: EntityId, value: f64 },
    /// A circle or arc has a specific radius.
    Radius { entity: EntityId, value: f64 },
    /// Angle of a line (relative to X-axis).
    LineAngle { entity: EntityId, angle: f64 },
    /// Two lines are perpendicular.
    Perpendicular { line_a: EntityId, line_b: EntityId },
    /// Two lines are parallel.
    Parallel { line_a: EntityId, line_b: EntityId },
    /// A point lies on a circle.
    PointOnCircle { point: PointRef, circle: EntityId },
    /// Two lines/arcs are equal in length/radius.
    Equal { a: EntityId, b: EntityId },
    /// A line is symmetric about X-axis.
    SymmetricX { a: PointRef, b: PointRef },
    /// A line is symmetric about Y-axis.
    SymmetricY { a: PointRef, b: PointRef },
    /// A point lies on a line entity.
    PointOnLine { point: PointRef, line: EntityId },
    /// Midpoint constraint — point is at midpoint of a line.
    Midpoint { point: PointRef, line: EntityId },
    /// Tangent: line is tangent to circle/arc at the point where they meet.
    /// entity_a = line entity ID, entity_b = circle/arc entity ID
    Tangent { entity_a: EntityId, entity_b: EntityId },
}

impl Constraint {
    /// Number of scalar equations this constraint contributes.
    pub fn equation_count(&self) -> usize {
        match self {
            Self::Fixed { .. } => 2,
            Self::Coincident { .. } => 2,
            Self::Horizontal { .. } => 1,
            Self::Vertical { .. } => 1,
            Self::Distance { .. } => 1,
            Self::LineLength { .. } => 1,
            Self::Radius { .. } => 1,
            Self::LineAngle { .. } => 1,
            Self::Perpendicular { .. } => 1,
            Self::Parallel { .. } => 1,
            Self::PointOnCircle { .. } => 1,
            Self::Equal { .. } => 1,
            Self::SymmetricX { .. } => 2,
            Self::SymmetricY { .. } => 2,
            Self::PointOnLine { .. } => 1,
            Self::Midpoint { .. } => 2,
            Self::Tangent { .. } => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equation_counts() {
        let c = Constraint::Fixed { point: PointRef::new(0, 0), x: 0.0, y: 0.0 };
        assert_eq!(c.equation_count(), 2);

        let c = Constraint::Distance { a: PointRef::new(0, 0), b: PointRef::new(1, 0), value: 5.0 };
        assert_eq!(c.equation_count(), 1);
    }
}
