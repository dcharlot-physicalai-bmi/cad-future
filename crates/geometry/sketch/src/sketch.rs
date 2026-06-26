//! Sketch — container for entities and constraints.

use serde::{Serialize, Deserialize};
use crate::entity::SketchEntity;
use crate::constraint::Constraint;

/// A 2D sketch with entities and constraints.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sketch {
    pub entities: Vec<SketchEntity>,
    pub constraints: Vec<Constraint>,
    /// Plane definition: origin, u-axis, v-axis, normal.
    pub plane_origin: [f64; 3],
    pub plane_u: [f64; 3],
    pub plane_v: [f64; 3],
    pub plane_normal: [f64; 3],
}

impl Sketch {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            constraints: Vec::new(),
            // Default: XY plane
            plane_origin: [0.0, 0.0, 0.0],
            plane_u: [1.0, 0.0, 0.0],
            plane_v: [0.0, 1.0, 0.0],
            plane_normal: [0.0, 0.0, 1.0],
        }
    }

    /// Create a sketch on the XZ plane (Front view).
    pub fn on_xz() -> Self {
        let mut s = Self::new();
        s.plane_u = [1.0, 0.0, 0.0];
        s.plane_v = [0.0, 0.0, 1.0];
        s.plane_normal = [0.0, 1.0, 0.0];
        s
    }

    /// Create a sketch on the YZ plane (Right view).
    pub fn on_yz() -> Self {
        let mut s = Self::new();
        s.plane_u = [0.0, 1.0, 0.0];
        s.plane_v = [0.0, 0.0, 1.0];
        s.plane_normal = [1.0, 0.0, 0.0];
        s
    }

    pub fn add_entity(&mut self, entity: SketchEntity) -> usize {
        let id = self.entities.len();
        self.entities.push(entity);
        id
    }

    pub fn add_constraint(&mut self, constraint: Constraint) -> usize {
        let id = self.constraints.len();
        self.constraints.push(constraint);
        id
    }

    /// Total parameter count across all entities.
    pub fn total_params(&self) -> usize {
        self.entities.iter().map(|e| e.param_count()).sum()
    }

    /// Total equation count across all constraints.
    pub fn total_equations(&self) -> usize {
        self.constraints.iter().map(|c| c.equation_count()).sum()
    }

    /// Degrees of freedom.
    pub fn dof(&self) -> i32 {
        self.total_params() as i32 - self.total_equations() as i32
    }

    /// Get the parameter offset for entity at index `idx`.
    pub fn param_offset(&self, idx: usize) -> usize {
        self.entities[..idx].iter().map(|e| e.param_count()).sum()
    }

    /// Collect all parameters into a flat vector.
    pub fn collect_params(&self) -> Vec<f64> {
        let mut params = Vec::with_capacity(self.total_params());
        for e in &self.entities {
            params.extend_from_slice(&e.params());
        }
        params
    }

    /// Get all point references in the sketch (for auto-constraint detection).
    pub fn all_point_refs(&self) -> Vec<crate::entity::PointRef> {
        use crate::entity::PointRef;
        let mut refs = Vec::new();
        for (idx, entity) in self.entities.iter().enumerate() {
            match entity {
                SketchEntity::Point { .. } => {
                    refs.push(PointRef::new(idx, 0));
                }
                SketchEntity::Line { .. } => {
                    refs.push(PointRef::new(idx, 0));
                    refs.push(PointRef::new(idx, 1));
                }
                SketchEntity::Circle { .. } => {
                    refs.push(PointRef::new(idx, 0)); // center
                }
                SketchEntity::Arc { .. } => {
                    refs.push(PointRef::new(idx, 0)); // center
                    refs.push(PointRef::new(idx, 1)); // start
                    refs.push(PointRef::new(idx, 2)); // end
                }
            }
        }
        refs
    }

    /// Apply a flat parameter vector back to entities.
    pub fn apply_params(&mut self, params: &[f64]) {
        let mut offset = 0;
        for e in &mut self.entities {
            let count = e.param_count();
            e.set_params(&params[offset..offset + count]);
            offset += count;
        }
    }
}

impl Default for Sketch {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_sketch() {
        let s = Sketch::new();
        assert_eq!(s.total_params(), 0);
        assert_eq!(s.total_equations(), 0);
        assert_eq!(s.dof(), 0);
    }

    #[test]
    fn param_collection() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(1.0, 2.0));
        s.add_entity(SketchEntity::line(3.0, 4.0, 5.0, 6.0));
        let params = s.collect_params();
        assert_eq!(params, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn dof_calculation() {
        let mut s = Sketch::new();
        s.add_entity(SketchEntity::point(0.0, 0.0));
        assert_eq!(s.dof(), 2);
        s.add_constraint(Constraint::Fixed {
            point: crate::entity::PointRef::new(0, 0), x: 0.0, y: 0.0,
        });
        assert_eq!(s.dof(), 0);
    }
}
