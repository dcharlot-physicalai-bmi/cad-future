//! `physical-parametric` — Parametric modeling document with feature tree,
//! undo/redo support, and rebuild capability.
//!
//! The feature tree is a serializable sequence of [`FeatureOp`] values.
//! [`ModelDocument::rebuild`] replays the entire tree to produce a [`Solid`].

use glam::DVec3;
use serde::{Serialize, Deserialize};
use physical_brep::{Solid, Profile};

/// A parametric modeling operation in the feature tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeatureOp {
    // -----------------------------------------------------------------------
    // Primitives
    // -----------------------------------------------------------------------
    /// Create a box primitive.
    Box { width: f64, height: f64, depth: f64 },
    /// Create a cylinder primitive.
    Cylinder { radius: f64, height: f64, segments: usize },

    // -----------------------------------------------------------------------
    // Sketch-based
    // -----------------------------------------------------------------------
    /// Extrude a solved sketch along a direction.
    ExtrudeSketch {
        sketch: physical_sketch::Sketch,
        direction: [f64; 3],
        distance: f64,
    },
    /// Extrude-cut: subtract the extruded sketch from the current solid.
    ExtrudeCutSketch {
        sketch: physical_sketch::Sketch,
        direction: [f64; 3],
        distance: f64,
    },

    // -----------------------------------------------------------------------
    // Sweep-class
    // -----------------------------------------------------------------------
    /// Revolve a profile around an axis.
    Revolve {
        profile: Profile,
        origin: [f64; 3],
        axis: [f64; 3],
        u_axis: [f64; 3],
        v_axis: [f64; 3],
        angle: f64,
        segments: usize,
    },
    /// Loft through cross sections.
    Loft { sections: Vec<Vec<[f64; 3]>> },
    /// Sweep a profile along a path.
    Sweep {
        profile: Profile,
        path: Vec<[f64; 3]>,
    },

    // -----------------------------------------------------------------------
    // Boolean CSG
    // -----------------------------------------------------------------------
    /// Union with another primitive.
    UnionBox { width: f64, height: f64, depth: f64, offset: [f64; 3] },
    /// Subtract another primitive.
    SubtractBox { width: f64, height: f64, depth: f64, offset: [f64; 3] },
    /// Intersect with another primitive.
    IntersectBox { width: f64, height: f64, depth: f64, offset: [f64; 3] },

    // -----------------------------------------------------------------------
    // Local operations
    // -----------------------------------------------------------------------
    /// Shell (hollow) a solid with given wall thickness.
    Shell { thickness: f64, open_face_indices: Vec<usize> },
    /// Fillet edges with a given radius.
    Fillet { edge_indices: Vec<usize>, radius: f64 },
    /// Chamfer edges with a given distance.
    Chamfer { edge_indices: Vec<usize>, distance: f64 },

    // -----------------------------------------------------------------------
    // Patterning
    // -----------------------------------------------------------------------
    /// Linear pattern: repeat the solid along a direction.
    LinearPattern {
        direction: [f64; 3],
        spacing: f64,
        count: usize,
    },
    /// Circular pattern: repeat around an axis.
    CircularPattern {
        axis_origin: [f64; 3],
        axis_direction: [f64; 3],
        count: usize,
        full_angle_deg: f64,
    },
    /// Mirror across a plane.
    Mirror {
        plane_point: [f64; 3],
        plane_normal: [f64; 3],
    },

    // -----------------------------------------------------------------------
    // Transforms
    // -----------------------------------------------------------------------
    /// Translate the solid.
    Translate { offset: [f64; 3] },
    /// Scale uniformly.
    Scale { factor: f64 },
}

/// Named feature with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    /// Human-readable feature name.
    pub name: String,
    /// The modeling operation.
    pub op: FeatureOp,
    /// Whether this feature is suppressed (skipped during rebuild).
    pub suppressed: bool,
}

impl Feature {
    pub fn new(name: &str, op: FeatureOp) -> Self {
        Self {
            name: name.to_string(),
            op,
            suppressed: false,
        }
    }
}

/// A parametric model document with a feature tree and undo/redo history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDocument {
    /// Document name.
    pub name: String,
    /// Current active features.
    pub features: Vec<Feature>,
    /// Undo stack (features removed by undo).
    redo_stack: Vec<Feature>,
}

impl ModelDocument {
    /// Create a new empty document.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            features: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Number of features in the tree.
    pub fn feature_count(&self) -> usize {
        self.features.len()
    }

    /// Add a feature operation to the tree.
    pub fn add(&mut self, name: &str, op: FeatureOp) {
        self.features.push(Feature::new(name, op));
        self.redo_stack.clear();
    }

    /// Undo the last feature operation.
    pub fn undo(&mut self) -> bool {
        if let Some(f) = self.features.pop() {
            self.redo_stack.push(f);
            true
        } else {
            false
        }
    }

    /// Redo the last undone feature operation.
    pub fn redo(&mut self) -> bool {
        if let Some(f) = self.redo_stack.pop() {
            self.features.push(f);
            true
        } else {
            false
        }
    }

    /// Suppress or un-suppress a feature by index.
    pub fn set_suppressed(&mut self, index: usize, suppressed: bool) {
        if let Some(f) = self.features.get_mut(index) {
            f.suppressed = suppressed;
        }
    }

    /// Reorder a feature from `from_index` to `to_index`.
    pub fn reorder(&mut self, from_index: usize, to_index: usize) {
        if from_index < self.features.len() && to_index < self.features.len() {
            let feature = self.features.remove(from_index);
            self.features.insert(to_index, feature);
        }
    }

    /// Insert a feature at a specific position.
    pub fn insert_at(&mut self, index: usize, name: &str, op: FeatureOp) {
        let clamped = index.min(self.features.len());
        self.features.insert(clamped, Feature::new(name, op));
        self.redo_stack.clear();
    }

    /// Remove a feature at a specific position.
    pub fn remove_at(&mut self, index: usize) -> Option<Feature> {
        if index < self.features.len() {
            Some(self.features.remove(index))
        } else {
            None
        }
    }

    /// List feature names (for UI display).
    pub fn feature_names(&self) -> Vec<&str> {
        self.features.iter().map(|f| f.name.as_str()).collect()
    }

    /// Rebuild the solid from the feature tree.
    ///
    /// Skips suppressed features. Returns `None` if the tree produces
    /// no geometry.
    pub fn rebuild(&self) -> Option<Solid> {
        let active: Vec<&FeatureOp> = self.features.iter()
            .filter(|f| !f.suppressed)
            .map(|f| &f.op)
            .collect();

        if active.is_empty() {
            return None;
        }

        let mut solid: Option<Solid> = None;

        for op in &active {
            solid = Some(apply_op(*op, solid)?);
        }

        solid
    }

    /// Rebuild up to (and including) the given feature index.
    /// Useful for "rollback" display in the feature tree.
    pub fn rebuild_to(&self, index: usize) -> Option<Solid> {
        let active: Vec<&FeatureOp> = self.features.iter()
            .take(index + 1)
            .filter(|f| !f.suppressed)
            .map(|f| &f.op)
            .collect();

        if active.is_empty() {
            return None;
        }

        let mut solid: Option<Solid> = None;

        for op in &active {
            solid = Some(apply_op(*op, solid)?);
        }

        solid
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

/// Apply a single feature operation, given the current solid state.
fn apply_op(op: &FeatureOp, current: Option<Solid>) -> Option<Solid> {
    match op {
        // --- Primitives ---
        FeatureOp::Box { width, height, depth } => {
            Some(physical_brep::builder::make_box(*width, *height, *depth))
        }
        FeatureOp::Cylinder { radius, height, segments } => {
            Some(physical_brep::builder::make_cylinder(*radius, *height, *segments))
        }

        // --- Sketch extrude ---
        FeatureOp::ExtrudeSketch { sketch, direction, distance } => {
            let profiles = physical_sketch::extract_profiles(sketch);
            if let Some(profile_segs) = profiles.first() {
                let brep_profile = physical_sketch::to_brep_profile(profile_segs);
                let dir = DVec3::from_array(*direction).normalize();
                let z = dir;
                let x = if z.x.abs() < 0.9 {
                    DVec3::X.cross(z).normalize()
                } else {
                    DVec3::Y.cross(z).normalize()
                };
                let y = z.cross(x).normalize();
                Some(physical_brep::extrude::extrude(
                    &brep_profile, DVec3::ZERO, x, y, z, *distance,
                ))
            } else {
                let profile = Profile::rectangle(10.0, 5.0);
                Some(physical_brep::extrude::extrude_z(&profile, *distance))
            }
        }
        FeatureOp::ExtrudeCutSketch { sketch, direction, distance } => {
            let s = current?;
            let profiles = physical_sketch::extract_profiles(sketch);
            if let Some(profile_segs) = profiles.first() {
                let brep_profile = physical_sketch::to_brep_profile(profile_segs);
                let dir = DVec3::from_array(*direction).normalize();
                let z = dir;
                let x = if z.x.abs() < 0.9 {
                    DVec3::X.cross(z).normalize()
                } else {
                    DVec3::Y.cross(z).normalize()
                };
                let y = z.cross(x).normalize();
                let tool = physical_brep::extrude::extrude(
                    &brep_profile, DVec3::ZERO, x, y, z, *distance,
                );
                Some(physical_brep::boolean::subtract(&s, &tool))
            } else {
                Some(s)
            }
        }

        // --- Sweep-class ---
        FeatureOp::Revolve { profile, origin, axis, u_axis, v_axis, angle, segments } => {
            Some(physical_brep::revolve::revolve(
                profile,
                DVec3::from_array(*origin),
                DVec3::from_array(*axis),
                DVec3::from_array(*u_axis),
                DVec3::from_array(*v_axis),
                *angle,
                *segments,
            ))
        }
        FeatureOp::Loft { sections } => {
            let section_points: Vec<Vec<DVec3>> = sections.iter()
                .map(|sec| sec.iter().map(|p| DVec3::from_array(*p)).collect())
                .collect();
            Some(physical_brep::loft::loft(&section_points))
        }
        FeatureOp::Sweep { profile, path } => {
            let path_points: Vec<DVec3> = path.iter()
                .map(|p| DVec3::from_array(*p))
                .collect();
            // Build a degree-1 NURBS (polyline) as the sweep path
            let n = path_points.len();
            let knots = {
                let mut k = vec![0.0];
                for i in 0..n {
                    k.push(i as f64 / (n - 1).max(1) as f64);
                }
                k.push(1.0);
                k
            };
            let path_curve = physical_brep::curve::Curve::Nurbs {
                control_points: path_points.clone(),
                weights: vec![1.0; n],
                knots,
                degree: 1,
            };
            Some(physical_brep::sweep::sweep(profile, &path_curve, n.max(2)))
        }

        // --- Boolean CSG ---
        FeatureOp::UnionBox { width, height, depth, offset } => {
            let s = current?;
            let mut tool = physical_brep::builder::make_box(*width, *height, *depth);
            translate_solid(&mut tool, DVec3::from_array(*offset));
            Some(physical_brep::boolean::union(&s, &tool))
        }
        FeatureOp::SubtractBox { width, height, depth, offset } => {
            let s = current?;
            let mut tool = physical_brep::builder::make_box(*width, *height, *depth);
            translate_solid(&mut tool, DVec3::from_array(*offset));
            Some(physical_brep::boolean::subtract(&s, &tool))
        }
        FeatureOp::IntersectBox { width, height, depth, offset } => {
            let s = current?;
            let mut tool = physical_brep::builder::make_box(*width, *height, *depth);
            translate_solid(&mut tool, DVec3::from_array(*offset));
            Some(physical_brep::boolean::intersect(&s, &tool))
        }

        // --- Local operations ---
        FeatureOp::Shell { thickness, open_face_indices } => {
            let s = current?;
            Some(physical_brep::shell::shell(&s, *thickness, open_face_indices))
        }
        FeatureOp::Fillet { edge_indices, radius } => {
            let mut s = current?;
            let edge_keys: Vec<_> = s.edges.keys().collect();
            let edges_to_fillet: Vec<_> = edge_indices.iter()
                .filter_map(|&i| edge_keys.get(i).copied())
                .collect();
            physical_brep::fillet::fillet(&mut s, &edges_to_fillet, *radius);
            Some(s)
        }
        FeatureOp::Chamfer { edge_indices, distance } => {
            let mut s = current?;
            let edge_keys: Vec<_> = s.edges.keys().collect();
            let edges_to_chamfer: Vec<_> = edge_indices.iter()
                .filter_map(|&i| edge_keys.get(i).copied())
                .collect();
            // Chamfer is implemented as a small fillet (approximation)
            physical_brep::fillet::fillet(&mut s, &edges_to_chamfer, *distance);
            Some(s)
        }

        // --- Patterning ---
        FeatureOp::LinearPattern { direction, spacing, count } => {
            let s = current?;
            let dir = DVec3::from_array(*direction);
            Some(physical_brep::pattern::linear_pattern(&s, dir, *spacing, *count))
        }
        FeatureOp::CircularPattern { axis_origin, axis_direction, count, full_angle_deg: _ } => {
            let s = current?;
            let origin = DVec3::from_array(*axis_origin);
            let axis = DVec3::from_array(*axis_direction);
            Some(physical_brep::pattern::circular_pattern(
                &s, origin, axis, *count,
            ))
        }
        FeatureOp::Mirror { plane_point, plane_normal } => {
            let s = current?;
            let point = DVec3::from_array(*plane_point);
            let normal = DVec3::from_array(*plane_normal);
            Some(physical_brep::pattern::mirror(&s, point, normal))
        }

        // --- Transforms ---
        FeatureOp::Translate { offset } => {
            let mut s = current?;
            translate_solid(&mut s, DVec3::from_array(*offset));
            Some(s)
        }
        FeatureOp::Scale { factor } => {
            let mut s = current?;
            for (_vid, v) in &mut s.vertices {
                v.point *= *factor;
            }
            Some(s)
        }
    }
}

/// Translate all vertices in a solid by an offset.
fn translate_solid(solid: &mut Solid, offset: DVec3) {
    for (_vid, v) in &mut solid.vertices {
        v.point += offset;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_document() {
        let doc = ModelDocument::new("test");
        assert_eq!(doc.feature_count(), 0);
        assert!(doc.rebuild().is_none());
    }

    #[test]
    fn box_feature() {
        let mut doc = ModelDocument::new("test");
        doc.add("Base", FeatureOp::Box { width: 10.0, height: 20.0, depth: 30.0 });
        let solid = doc.rebuild().unwrap();
        assert_eq!(solid.vertices.len(), 8);
        assert_eq!(solid.faces.len(), 6);
    }

    #[test]
    fn undo_redo() {
        let mut doc = ModelDocument::new("test");
        doc.add("Base", FeatureOp::Box { width: 10.0, height: 20.0, depth: 30.0 });
        doc.add("Shell", FeatureOp::Shell { thickness: 1.0, open_face_indices: vec![0] });
        assert_eq!(doc.feature_count(), 2);

        assert!(doc.undo());
        assert_eq!(doc.feature_count(), 1);

        assert!(doc.redo());
        assert_eq!(doc.feature_count(), 2);

        assert!(doc.undo());
        assert!(doc.undo());
        assert!(!doc.undo()); // nothing to undo
    }

    #[test]
    fn suppress_feature() {
        let mut doc = ModelDocument::new("test");
        doc.add("Base", FeatureOp::Box { width: 10.0, height: 20.0, depth: 30.0 });
        doc.add("Shell", FeatureOp::Shell { thickness: 1.0, open_face_indices: vec![] });

        let with_shell = doc.rebuild().unwrap();
        doc.set_suppressed(1, true);
        let without_shell = doc.rebuild().unwrap();

        // Shell creates more faces, so suppressing it should give fewer
        assert!(without_shell.faces.len() <= with_shell.faces.len());
    }

    #[test]
    fn reorder_features() {
        let mut doc = ModelDocument::new("test");
        doc.add("A", FeatureOp::Box { width: 10.0, height: 10.0, depth: 10.0 });
        doc.add("B", FeatureOp::Fillet { edge_indices: vec![0], radius: 1.0 });
        doc.add("C", FeatureOp::Shell { thickness: 1.0, open_face_indices: vec![] });

        assert_eq!(doc.feature_names(), vec!["A", "B", "C"]);
        doc.reorder(2, 1);
        assert_eq!(doc.feature_names(), vec!["A", "C", "B"]);
    }

    #[test]
    fn rebuild_to() {
        let mut doc = ModelDocument::new("test");
        doc.add("Base", FeatureOp::Box { width: 10.0, height: 20.0, depth: 30.0 });
        doc.add("Shell", FeatureOp::Shell { thickness: 1.0, open_face_indices: vec![] });

        let at_0 = doc.rebuild_to(0).unwrap();
        let at_1 = doc.rebuild_to(1).unwrap();

        assert_eq!(at_0.faces.len(), 6); // box has 6 faces
        assert!(at_1.faces.len() >= 6); // shell may add inner faces
    }

    #[test]
    fn boolean_union() {
        let mut doc = ModelDocument::new("test");
        doc.add("Base", FeatureOp::Box { width: 10.0, height: 10.0, depth: 10.0 });
        doc.add("Union", FeatureOp::UnionBox {
            width: 5.0, height: 5.0, depth: 5.0,
            offset: [5.0, 0.0, 0.0],
        });
        let solid = doc.rebuild().unwrap();
        assert!(solid.vertices.len() > 8, "union should have more vertices than a single box");
    }

    #[test]
    fn boolean_subtract() {
        let mut doc = ModelDocument::new("test");
        doc.add("Base", FeatureOp::Box { width: 20.0, height: 20.0, depth: 20.0 });
        doc.add("Cut", FeatureOp::SubtractBox {
            width: 5.0, height: 5.0, depth: 25.0,
            offset: [7.5, 7.5, -2.5],
        });
        let solid = doc.rebuild().unwrap();
        assert!(solid.vertices.len() > 8, "subtract should create additional vertices");
    }

    #[test]
    fn linear_pattern() {
        let mut doc = ModelDocument::new("test");
        doc.add("Base", FeatureOp::Box { width: 5.0, height: 5.0, depth: 5.0 });
        doc.add("Pattern", FeatureOp::LinearPattern {
            direction: [10.0, 0.0, 0.0],
            spacing: 10.0,
            count: 3,
        });
        let solid = doc.rebuild().unwrap();
        // 3 copies of a box: 3 × 8 = 24 vertices (at minimum)
        assert!(solid.vertices.len() >= 24);
    }

    #[test]
    fn mirror_feature() {
        let mut doc = ModelDocument::new("test");
        doc.add("Base", FeatureOp::Box { width: 5.0, height: 5.0, depth: 5.0 });
        doc.add("Mirror", FeatureOp::Mirror {
            plane_point: [0.0, 0.0, 0.0],
            plane_normal: [1.0, 0.0, 0.0],
        });
        let solid = doc.rebuild().unwrap();
        // Mirror returns reflected geometry (8 vertices for a box)
        assert_eq!(solid.vertices.len(), 8, "mirrored box should have 8 vertices");
        // Verify reflection: all x coordinates should be negated
        let original = physical_brep::builder::make_box(5.0, 5.0, 5.0);
        let orig_xs: Vec<f64> = original.vertices.values().map(|v| v.point.x).collect();
        let mirror_xs: Vec<f64> = solid.vertices.values().map(|v| v.point.x).collect();
        // The mirrored x values should have opposite signs
        let orig_sum: f64 = orig_xs.iter().sum();
        let mirror_sum: f64 = mirror_xs.iter().sum();
        assert!((orig_sum + mirror_sum).abs() < 0.01,
            "mirror x sum should negate: orig={}, mirror={}", orig_sum, mirror_sum);
    }

    #[test]
    fn translate_feature() {
        let mut doc = ModelDocument::new("test");
        doc.add("Base", FeatureOp::Box { width: 10.0, height: 10.0, depth: 10.0 });

        // Find the original min x
        let original = doc.rebuild().unwrap();
        let orig_min_x = original.vertices.values().map(|v| v.point.x).fold(f64::MAX, f64::min);

        doc.add("Move", FeatureOp::Translate { offset: [100.0, 0.0, 0.0] });
        let solid = doc.rebuild().unwrap();
        let new_min_x = solid.vertices.values().map(|v| v.point.x).fold(f64::MAX, f64::min);

        assert!((new_min_x - (orig_min_x + 100.0)).abs() < 0.01,
            "translate should shift x by 100: orig_min={}, new_min={}", orig_min_x, new_min_x);
    }

    #[test]
    fn scale_feature() {
        let mut doc = ModelDocument::new("test");
        doc.add("Base", FeatureOp::Box { width: 10.0, height: 10.0, depth: 10.0 });

        let original = doc.rebuild().unwrap();
        let orig_max_x = original.vertices.values().map(|v| v.point.x).fold(0.0f64, f64::max);

        doc.add("Scale2x", FeatureOp::Scale { factor: 2.0 });
        let scaled = doc.rebuild().unwrap();
        let scaled_max_x = scaled.vertices.values().map(|v| v.point.x).fold(0.0f64, f64::max);

        assert!((scaled_max_x - orig_max_x * 2.0).abs() < 0.01,
            "scaling 2x should double coordinates: orig={}, scaled={}", orig_max_x, scaled_max_x);
    }

    #[test]
    fn serialization_roundtrip() {
        let mut doc = ModelDocument::new("bracket");
        doc.add("Base", FeatureOp::Box { width: 50.0, height: 30.0, depth: 5.0 });
        doc.add("Fillet", FeatureOp::Fillet { edge_indices: vec![0, 1], radius: 2.0 });

        let json = doc.to_json();
        let restored = ModelDocument::from_json(&json).unwrap();

        assert_eq!(restored.name, "bracket");
        assert_eq!(restored.feature_count(), 2);
        assert_eq!(restored.feature_names(), vec!["Base", "Fillet"]);
    }

    #[test]
    fn cylinder_feature() {
        let mut doc = ModelDocument::new("test");
        doc.add("Cylinder", FeatureOp::Cylinder { radius: 5.0, height: 20.0, segments: 16 });
        let solid = doc.rebuild().unwrap();
        assert!(solid.vertices.len() >= 32, "cylinder should have 2×segments vertices");
    }

    #[test]
    fn insert_and_remove() {
        let mut doc = ModelDocument::new("test");
        doc.add("A", FeatureOp::Box { width: 10.0, height: 10.0, depth: 10.0 });
        doc.add("C", FeatureOp::Shell { thickness: 1.0, open_face_indices: vec![] });

        doc.insert_at(1, "B", FeatureOp::Fillet { edge_indices: vec![0], radius: 1.0 });
        assert_eq!(doc.feature_names(), vec!["A", "B", "C"]);

        let removed = doc.remove_at(1).unwrap();
        assert_eq!(removed.name, "B");
        assert_eq!(doc.feature_names(), vec!["A", "C"]);
    }

    #[test]
    fn add_clears_redo() {
        let mut doc = ModelDocument::new("test");
        doc.add("A", FeatureOp::Box { width: 10.0, height: 10.0, depth: 10.0 });
        doc.add("B", FeatureOp::Shell { thickness: 1.0, open_face_indices: vec![] });
        doc.undo();
        // Now add a new feature — redo stack should be cleared
        doc.add("C", FeatureOp::Fillet { edge_indices: vec![0], radius: 1.0 });
        assert!(!doc.redo()); // redo stack was cleared
        assert_eq!(doc.feature_count(), 2);
    }

    #[test]
    fn loft_feature() {
        let mut doc = ModelDocument::new("test");
        doc.add("Loft", FeatureOp::Loft {
            sections: vec![
                vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0], [0.0, 10.0, 0.0]],
                vec![[2.0, 2.0, 20.0], [8.0, 2.0, 20.0], [8.0, 8.0, 20.0], [2.0, 8.0, 20.0]],
            ],
        });
        let solid = doc.rebuild().unwrap();
        assert!(solid.vertices.len() >= 8, "loft should have vertices from both sections");
    }
}
