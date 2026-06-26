//! Assembly constraint solver — mates, DOF tracking, interference detection.
//!
//! Reuses the Newton-Raphson pattern from the sketch constraint solver.
//! Geometric constraints position rigid bodies (parts) in 3D space.
//! Each part has 6 DOF: 3 translational + 3 rotational (Euler angles).

use glam::{DVec3, DMat3, DQuat};
use serde::{Serialize, Deserialize};

use crate::solid::Solid;

// ---------------------------------------------------------------------------
// Part Placement
// ---------------------------------------------------------------------------

/// A rigid body placement in 3D — position + orientation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Placement {
    /// Translation from world origin.
    pub position: DVec3,
    /// Orientation as quaternion (for interpolation and composition).
    pub orientation: DQuat,
}

impl Placement {
    pub fn identity() -> Self {
        Self {
            position: DVec3::ZERO,
            orientation: DQuat::IDENTITY,
        }
    }

    pub fn from_position(pos: DVec3) -> Self {
        Self {
            position: pos,
            orientation: DQuat::IDENTITY,
        }
    }

    /// Transform a point from part-local to world space.
    pub fn transform_point(&self, local: DVec3) -> DVec3 {
        self.orientation * local + self.position
    }

    /// Transform a direction from part-local to world space.
    pub fn transform_direction(&self, local: DVec3) -> DVec3 {
        self.orientation * local
    }

    /// Rotation matrix (3x3).
    pub fn rotation_matrix(&self) -> DMat3 {
        DMat3::from_quat(self.orientation)
    }

    /// Pack placement into 6 parameters: [tx, ty, tz, rx, ry, rz].
    /// Rotation uses axis-angle representation (Rodrigues vector).
    pub fn to_params(&self) -> [f64; 6] {
        let (axis, angle) = self.orientation.to_axis_angle();
        let rod = axis * angle as f64;
        [
            self.position.x, self.position.y, self.position.z,
            rod.x, rod.y, rod.z,
        ]
    }

    /// Unpack from 6 parameters.
    pub fn from_params(params: &[f64; 6]) -> Self {
        let position = DVec3::new(params[0], params[1], params[2]);
        let rod = DVec3::new(params[3], params[4], params[5]);
        let angle = rod.length();
        let orientation = if angle > 1e-12 {
            DQuat::from_axis_angle(rod / angle, angle)
        } else {
            DQuat::IDENTITY
        };
        Self { position, orientation }
    }
}

impl Default for Placement {
    fn default() -> Self { Self::identity() }
}

// ---------------------------------------------------------------------------
// Assembly Part
// ---------------------------------------------------------------------------

/// A part instance within an assembly.
#[derive(Clone, Debug)]
pub struct AssemblyPart {
    /// Part name / label.
    pub name: String,
    /// The B-Rep solid.
    pub solid: Solid,
    /// Current placement in world space.
    pub placement: Placement,
    /// Whether this part is grounded (fixed in space, 0 DOF).
    pub grounded: bool,
}

impl AssemblyPart {
    pub fn new(name: &str, solid: Solid) -> Self {
        Self {
            name: name.to_string(),
            solid,
            placement: Placement::identity(),
            grounded: false,
        }
    }

    pub fn with_placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    pub fn grounded(mut self) -> Self {
        self.grounded = true;
        self
    }

    /// Get the bounding box in world space.
    pub fn world_bounding_box(&self) -> (DVec3, DVec3) {
        let (local_min, local_max) = self.solid.bounding_box();
        // Transform all 8 corners and find new AABB
        let corners = [
            DVec3::new(local_min.x, local_min.y, local_min.z),
            DVec3::new(local_max.x, local_min.y, local_min.z),
            DVec3::new(local_min.x, local_max.y, local_min.z),
            DVec3::new(local_max.x, local_max.y, local_min.z),
            DVec3::new(local_min.x, local_min.y, local_max.z),
            DVec3::new(local_max.x, local_min.y, local_max.z),
            DVec3::new(local_min.x, local_max.y, local_max.z),
            DVec3::new(local_max.x, local_max.y, local_max.z),
        ];
        let mut world_min = DVec3::splat(f64::MAX);
        let mut world_max = DVec3::splat(f64::MIN);
        for c in &corners {
            let w = self.placement.transform_point(*c);
            world_min = world_min.min(w);
            world_max = world_max.max(w);
        }
        (world_min, world_max)
    }
}

// ---------------------------------------------------------------------------
// Assembly Mate Constraints
// ---------------------------------------------------------------------------

/// Geometric reference on a part: a point, axis, or plane.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MateRef {
    /// A point in part-local coordinates.
    Point(DVec3),
    /// An axis: origin + direction in part-local coordinates.
    Axis { origin: DVec3, direction: DVec3 },
    /// A plane: point on plane + normal in part-local coordinates.
    Plane { point: DVec3, normal: DVec3 },
}

/// Assembly mate constraint type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MateType {
    /// Two points coincide.
    Coincident,
    /// Two axes are concentric (parallel + intersecting).
    Concentric,
    /// Two planes are coplanar (with optional offset).
    Coplanar { offset: f64 },
    /// Two directions are parallel.
    Parallel,
    /// Two directions are perpendicular.
    Perpendicular,
    /// Two entities are tangent (point on surface).
    Tangent,
    /// Fixed distance between two points.
    Distance { value: f64 },
    /// Fixed angle between two directions (radians).
    Angle { value: f64 },
}

/// A mate between two parts.
#[derive(Clone, Debug)]
pub struct Mate {
    /// Index of part A in the assembly.
    pub part_a: usize,
    /// Reference geometry on part A.
    pub ref_a: MateRef,
    /// Index of part B in the assembly.
    pub part_b: usize,
    /// Reference geometry on part B.
    pub ref_b: MateRef,
    /// Mate type.
    pub mate_type: MateType,
}

// ---------------------------------------------------------------------------
// Assembly
// ---------------------------------------------------------------------------

/// An assembly — collection of parts with mate constraints.
#[derive(Clone, Debug)]
pub struct Assembly {
    pub parts: Vec<AssemblyPart>,
    pub mates: Vec<Mate>,
}

/// Result of constraint solving.
#[derive(Clone, Debug, PartialEq)]
pub enum AssemblySolveResult {
    /// All constraints satisfied.
    FullyConstrained,
    /// Some DOF remain.
    UnderConstrained { remaining_dof: usize },
    /// Over-constrained (conflicting mates).
    OverConstrained,
    /// Solver failed to converge.
    Failed,
}

const MAX_ITER: usize = 100;
const TOLERANCE: f64 = 1e-8;

impl Assembly {
    pub fn new() -> Self {
        Self { parts: Vec::new(), mates: Vec::new() }
    }

    /// Add a part, returns its index.
    pub fn add_part(&mut self, part: AssemblyPart) -> usize {
        let idx = self.parts.len();
        self.parts.push(part);
        idx
    }

    /// Add a mate constraint.
    pub fn add_mate(&mut self, mate: Mate) {
        self.mates.push(mate);
    }

    /// Total free parameters (6 per non-grounded part).
    fn total_params(&self) -> usize {
        self.parts.iter().filter(|p| !p.grounded).count() * 6
    }

    /// Total constraint equations.
    fn total_equations(&self) -> usize {
        self.mates.iter().map(|m| mate_equation_count(&m.mate_type)).sum()
    }

    /// Collect all placement parameters into a vector.
    fn collect_params(&self) -> Vec<f64> {
        let mut params = Vec::with_capacity(self.total_params());
        for part in &self.parts {
            if !part.grounded {
                let p = part.placement.to_params();
                params.extend_from_slice(&p);
            }
        }
        params
    }

    /// Apply parameter vector back to placements.
    fn apply_params(&mut self, params: &[f64]) {
        let mut offset = 0;
        for part in &mut self.parts {
            if !part.grounded {
                let p: [f64; 6] = [
                    params[offset], params[offset + 1], params[offset + 2],
                    params[offset + 3], params[offset + 4], params[offset + 5],
                ];
                part.placement = Placement::from_params(&p);
                offset += 6;
            }
        }
    }

    /// Evaluate all mate constraint residuals.
    fn evaluate_residuals(&self) -> Vec<f64> {
        let mut residuals = Vec::new();

        for mate in &self.mates {
            let place_a = &self.parts[mate.part_a].placement;
            let place_b = &self.parts[mate.part_b].placement;

            match (&mate.ref_a, &mate.ref_b, &mate.mate_type) {
                // Coincident: two points must be at the same world position
                (MateRef::Point(pa), MateRef::Point(pb), MateType::Coincident) => {
                    let wa = place_a.transform_point(*pa);
                    let wb = place_b.transform_point(*pb);
                    residuals.push(wa.x - wb.x);
                    residuals.push(wa.y - wb.y);
                    residuals.push(wa.z - wb.z);
                }

                // Concentric: two axes parallel + intersection
                (MateRef::Axis { origin: oa, direction: da }, MateRef::Axis { origin: ob, direction: db }, MateType::Concentric) => {
                    let wa = place_a.transform_point(*oa);
                    let wb = place_b.transform_point(*ob);
                    let wda = place_a.transform_direction(*da).normalize();
                    let wdb = place_b.transform_direction(*db).normalize();

                    // Parallel: cross product = 0 (2 equations)
                    let cross = wda.cross(wdb);
                    residuals.push(cross.x);
                    residuals.push(cross.y);

                    // Intersecting: distance between axes = 0 (2 equations)
                    // Project (wb - wa) onto plane perpendicular to wda
                    let diff = wb - wa;
                    let along = diff.dot(wda);
                    let perp = diff - wda * along;
                    residuals.push(perp.x);
                    residuals.push(perp.y);
                }

                // Coplanar: two planes at same position (with offset)
                (MateRef::Plane { point: pa, normal: na }, MateRef::Plane { point: pb, normal: nb }, MateType::Coplanar { offset }) => {
                    let wa = place_a.transform_point(*pa);
                    let wna = place_a.transform_direction(*na).normalize();
                    let wb = place_b.transform_point(*pb);
                    let wnb = place_b.transform_direction(*nb).normalize();

                    // Normals anti-parallel (face-to-face): dot = -1
                    // or parallel: dot = 1. We enforce anti-parallel for mating faces.
                    residuals.push(wna.dot(wnb) + 1.0);

                    // Distance along normal = offset
                    let dist = (wb - wa).dot(wna);
                    residuals.push(dist - offset);
                }

                // Parallel: two directions must be parallel
                (MateRef::Axis { direction: da, .. }, MateRef::Axis { direction: db, .. }, MateType::Parallel)
                | (MateRef::Plane { normal: da, .. }, MateRef::Plane { normal: db, .. }, MateType::Parallel) => {
                    let wda = place_a.transform_direction(*da).normalize();
                    let wdb = place_b.transform_direction(*db).normalize();
                    let cross = wda.cross(wdb);
                    residuals.push(cross.x);
                    residuals.push(cross.y);
                    residuals.push(cross.z);
                }

                // Perpendicular: dot product = 0
                (MateRef::Axis { direction: da, .. }, MateRef::Axis { direction: db, .. }, MateType::Perpendicular)
                | (MateRef::Plane { normal: da, .. }, MateRef::Plane { normal: db, .. }, MateType::Perpendicular) => {
                    let wda = place_a.transform_direction(*da).normalize();
                    let wdb = place_b.transform_direction(*db).normalize();
                    residuals.push(wda.dot(wdb));
                }

                // Distance: two points at fixed distance
                (MateRef::Point(pa), MateRef::Point(pb), MateType::Distance { value }) => {
                    let wa = place_a.transform_point(*pa);
                    let wb = place_b.transform_point(*pb);
                    let dist_sq = (wa - wb).length_squared();
                    residuals.push(dist_sq - value * value);
                }

                // Angle: angle between two directions
                (MateRef::Axis { direction: da, .. }, MateRef::Axis { direction: db, .. }, MateType::Angle { value })
                | (MateRef::Plane { normal: da, .. }, MateRef::Plane { normal: db, .. }, MateType::Angle { value }) => {
                    let wda = place_a.transform_direction(*da).normalize();
                    let wdb = place_b.transform_direction(*db).normalize();
                    let cos_angle = wda.dot(wdb).clamp(-1.0, 1.0);
                    residuals.push(cos_angle - value.cos());
                }

                // Tangent: point lies on plane
                (MateRef::Point(pa), MateRef::Plane { point: pb, normal: nb }, MateType::Tangent) => {
                    let wa = place_a.transform_point(*pa);
                    let wb = place_b.transform_point(*pb);
                    let wn = place_b.transform_direction(*nb).normalize();
                    residuals.push((wa - wb).dot(wn));
                }

                _ => {} // unsupported combination — skip
            }
        }

        residuals
    }

    /// Solve the assembly constraints using Newton-Raphson.
    pub fn solve(&mut self) -> AssemblySolveResult {
        let n_params = self.total_params();
        let n_equations = self.total_equations();

        if n_equations == 0 {
            return AssemblySolveResult::UnderConstrained { remaining_dof: n_params };
        }
        if n_equations > n_params {
            return AssemblySolveResult::OverConstrained;
        }

        for _iter in 0..MAX_ITER {
            let residuals = self.evaluate_residuals();

            // Check convergence
            let max_r = residuals.iter().map(|r| r.abs()).fold(0.0_f64, f64::max);
            if max_r < TOLERANCE {
                let dof = n_params - n_equations;
                return if dof == 0 {
                    AssemblySolveResult::FullyConstrained
                } else {
                    AssemblySolveResult::UnderConstrained { remaining_dof: dof }
                };
            }

            // Build Jacobian via finite differences
            let x = self.collect_params();
            let jac = self.build_jacobian(&x, n_equations, n_params);

            // Solve J^T*J * delta = J^T * (-r) (normal equations)
            let jtj = mat_mul_ata(&jac, n_equations, n_params);
            let jtr = mat_vec_at(&jac, &residuals, n_equations, n_params);

            // Tikhonov regularization for under-constrained
            let mut jtj_reg = jtj;
            if n_equations < n_params {
                let lambda = 1e-8;
                for i in 0..n_params {
                    jtj_reg[i * n_params + i] += lambda;
                }
            }

            match solve_linear(&jtj_reg, &jtr, n_params) {
                Some(delta) => {
                    let mut new_x = x;
                    for i in 0..n_params {
                        new_x[i] += delta[i];
                    }
                    self.apply_params(&new_x);
                }
                None => return AssemblySolveResult::Failed,
            }
        }

        AssemblySolveResult::Failed
    }

    /// Build Jacobian via finite differences.
    fn build_jacobian(&mut self, params: &[f64], neq: usize, npar: usize) -> Vec<f64> {
        let eps = 1e-7;
        let r0 = self.evaluate_residuals();
        let mut jac = vec![0.0; neq * npar];

        for j in 0..npar {
            let mut perturbed = params.to_vec();
            perturbed[j] += eps;
            self.apply_params(&perturbed);
            let r1 = self.evaluate_residuals();
            for i in 0..neq {
                jac[i * npar + j] = (r1[i] - r0[i]) / eps;
            }
        }
        // Restore original params
        self.apply_params(params);
        jac
    }

    /// DOF analysis: total free DOF minus consumed by mates.
    pub fn remaining_dof(&self) -> usize {
        let free = self.total_params();
        let consumed = self.total_equations();
        free.saturating_sub(consumed)
    }

    /// Check for bounding-box interference between all part pairs.
    /// Returns pairs of interfering part indices.
    pub fn check_interference(&self) -> Vec<(usize, usize)> {
        let mut collisions = Vec::new();
        let n = self.parts.len();

        for i in 0..n {
            let (min_i, max_i) = self.parts[i].world_bounding_box();
            for j in (i + 1)..n {
                let (min_j, max_j) = self.parts[j].world_bounding_box();
                if aabb_overlap(min_i, max_i, min_j, max_j) {
                    collisions.push((i, j));
                }
            }
        }

        collisions
    }
}

impl Default for Assembly {
    fn default() -> Self { Self::new() }
}

/// Number of residual equations for a mate type.
fn mate_equation_count(mate_type: &MateType) -> usize {
    match mate_type {
        MateType::Coincident => 3,
        MateType::Concentric => 4,
        MateType::Coplanar { .. } => 2,
        MateType::Parallel => 3,
        MateType::Perpendicular => 1,
        MateType::Tangent => 1,
        MateType::Distance { .. } => 1,
        MateType::Angle { .. } => 1,
    }
}

/// Check AABB overlap.
fn aabb_overlap(min_a: DVec3, max_a: DVec3, min_b: DVec3, max_b: DVec3) -> bool {
    min_a.x <= max_b.x && max_a.x >= min_b.x
        && min_a.y <= max_b.y && max_a.y >= min_b.y
        && min_a.z <= max_b.z && max_a.z >= min_b.z
}

// ---------------------------------------------------------------------------
// Linear algebra (same pattern as sketch solver)
// ---------------------------------------------------------------------------

/// A^T * A (npar × npar).
fn mat_mul_ata(a: &[f64], nrows: usize, ncols: usize) -> Vec<f64> {
    let mut result = vec![0.0; ncols * ncols];
    for r in 0..ncols {
        for c in 0..ncols {
            let mut sum = 0.0;
            for k in 0..nrows {
                sum += a[k * ncols + r] * a[k * ncols + c];
            }
            result[r * ncols + c] = sum;
        }
    }
    result
}

/// A^T * (-v) → npar vector.
fn mat_vec_at(a: &[f64], v: &[f64], nrows: usize, ncols: usize) -> Vec<f64> {
    let mut result = vec![0.0; ncols];
    for r in 0..ncols {
        let mut sum = 0.0;
        for k in 0..nrows {
            sum += a[k * ncols + r] * (-v[k]);
        }
        result[r] = sum;
    }
    result
}

/// Solve Ax = b via Gaussian elimination with partial pivoting.
fn solve_linear(a: &[f64], b: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut aug = vec![0.0; n * (n + 1)];
    for r in 0..n {
        for c in 0..n {
            aug[r * (n + 1) + c] = a[r * n + c];
        }
        aug[r * (n + 1) + n] = b[r];
    }

    for col in 0..n {
        let mut max_val = aug[col * (n + 1) + col].abs();
        let mut max_row = col;
        for row in (col + 1)..n {
            let val = aug[row * (n + 1) + col].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }
        if max_val < 1e-14 { return None; }

        if max_row != col {
            for c in 0..=n {
                let tmp = aug[col * (n + 1) + c];
                aug[col * (n + 1) + c] = aug[max_row * (n + 1) + c];
                aug[max_row * (n + 1) + c] = tmp;
            }
        }

        let pivot = aug[col * (n + 1) + col];
        for row in (col + 1)..n {
            let factor = aug[row * (n + 1) + col] / pivot;
            for c in col..=n {
                aug[row * (n + 1) + c] -= factor * aug[col * (n + 1) + c];
            }
        }
    }

    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let mut sum = aug[row * (n + 1) + n];
        for c in (row + 1)..n {
            sum -= aug[row * (n + 1) + c] * x[c];
        }
        let diag = aug[row * (n + 1) + row];
        if diag.abs() < 1e-14 { return None; }
        x[row] = sum / diag;
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::make_box;

    #[test]
    fn placement_identity() {
        let p = Placement::identity();
        let pt = p.transform_point(DVec3::new(1.0, 2.0, 3.0));
        assert!((pt - DVec3::new(1.0, 2.0, 3.0)).length() < 1e-10);
    }

    #[test]
    fn placement_translation() {
        let p = Placement::from_position(DVec3::new(10.0, 0.0, 0.0));
        let pt = p.transform_point(DVec3::ZERO);
        assert!((pt - DVec3::new(10.0, 0.0, 0.0)).length() < 1e-10);
    }

    #[test]
    fn placement_roundtrip() {
        let p = Placement {
            position: DVec3::new(1.0, 2.0, 3.0),
            orientation: DQuat::from_rotation_z(std::f64::consts::FRAC_PI_4),
        };
        let params = p.to_params();
        let p2 = Placement::from_params(&params);
        let test_pt = DVec3::new(5.0, 0.0, 0.0);
        let diff = (p.transform_point(test_pt) - p2.transform_point(test_pt)).length();
        assert!(diff < 1e-8, "roundtrip diff={}", diff);
    }

    #[test]
    fn empty_assembly() {
        let asm = Assembly::new();
        assert_eq!(asm.remaining_dof(), 0);
        assert!(asm.check_interference().is_empty());
    }

    #[test]
    fn single_grounded_part() {
        let mut asm = Assembly::new();
        let box1 = make_box(10.0, 10.0, 10.0);
        asm.add_part(AssemblyPart::new("base", box1).grounded());
        assert_eq!(asm.remaining_dof(), 0);
    }

    #[test]
    fn two_parts_no_mates() {
        let mut asm = Assembly::new();
        let box1 = make_box(10.0, 10.0, 10.0);
        let box2 = make_box(5.0, 5.0, 5.0);
        asm.add_part(AssemblyPart::new("base", box1).grounded());
        asm.add_part(AssemblyPart::new("part2", box2));
        assert_eq!(asm.remaining_dof(), 6);
    }

    #[test]
    fn coincident_mate_solve() {
        let mut asm = Assembly::new();
        let box1 = make_box(10.0, 10.0, 10.0);
        let box2 = make_box(5.0, 5.0, 5.0);

        let a = asm.add_part(AssemblyPart::new("base", box1).grounded());
        let b = asm.add_part(
            AssemblyPart::new("block", box2)
                .with_placement(Placement::from_position(DVec3::new(3.0, 2.0, 1.0)))
        );

        // Mate: origin of part B coincides with point (10,0,0) on part A
        asm.add_mate(Mate {
            part_a: a,
            ref_a: MateRef::Point(DVec3::new(10.0, 0.0, 0.0)),
            part_b: b,
            ref_b: MateRef::Point(DVec3::ZERO),
            mate_type: MateType::Coincident,
        });

        let result = asm.solve();
        // Should be under-constrained (3 equations, 6 DOF → 3 DOF remaining)
        assert!(matches!(result,
            AssemblySolveResult::UnderConstrained { remaining_dof: 3 }
        ), "result={:?}", result);

        // Part B origin should now be at (10, 0, 0) in world space
        let world_origin = asm.parts[b].placement.transform_point(DVec3::ZERO);
        let err = (world_origin - DVec3::new(10.0, 0.0, 0.0)).length();
        assert!(err < 1e-6, "position error={}", err);
    }

    #[test]
    fn interference_detection() {
        let mut asm = Assembly::new();
        let box1 = make_box(10.0, 10.0, 10.0);
        let box2 = make_box(10.0, 10.0, 10.0);

        // Overlapping boxes
        asm.add_part(AssemblyPart::new("a", box1).grounded());
        asm.add_part(
            AssemblyPart::new("b", box2)
                .with_placement(Placement::from_position(DVec3::new(5.0, 5.0, 5.0)))
        );

        let collisions = asm.check_interference();
        assert_eq!(collisions.len(), 1);
        assert_eq!(collisions[0], (0, 1));
    }

    #[test]
    fn no_interference_separated() {
        let mut asm = Assembly::new();
        let box1 = make_box(10.0, 10.0, 10.0);
        let box2 = make_box(10.0, 10.0, 10.0);

        asm.add_part(AssemblyPart::new("a", box1).grounded());
        asm.add_part(
            AssemblyPart::new("b", box2)
                .with_placement(Placement::from_position(DVec3::new(20.0, 0.0, 0.0)))
        );

        assert!(asm.check_interference().is_empty());
    }

    #[test]
    fn dof_tracking() {
        let mut asm = Assembly::new();
        let box1 = make_box(10.0, 10.0, 10.0);
        let box2 = make_box(5.0, 5.0, 5.0);

        let a = asm.add_part(AssemblyPart::new("base", box1).grounded());
        let b = asm.add_part(AssemblyPart::new("block", box2));

        assert_eq!(asm.remaining_dof(), 6); // free part

        // Add coincident → 6 - 3 = 3
        asm.add_mate(Mate {
            part_a: a,
            ref_a: MateRef::Point(DVec3::ZERO),
            part_b: b,
            ref_b: MateRef::Point(DVec3::ZERO),
            mate_type: MateType::Coincident,
        });
        assert_eq!(asm.remaining_dof(), 3);

        // Add parallel → 3 - 3 = 0
        asm.add_mate(Mate {
            part_a: a,
            ref_a: MateRef::Axis { origin: DVec3::ZERO, direction: DVec3::Z },
            part_b: b,
            ref_b: MateRef::Axis { origin: DVec3::ZERO, direction: DVec3::Z },
            mate_type: MateType::Parallel,
        });
        assert_eq!(asm.remaining_dof(), 0);
    }

    #[test]
    fn solve_linear_3x3() {
        let a = vec![1.0, 1.0, 1.0, 2.0, 1.0, -1.0, 1.0, -1.0, 1.0];
        let b = vec![6.0, 1.0, 2.0];
        let x = solve_linear(&a, &b, 3).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-10);
        assert!((x[1] - 2.0).abs() < 1e-10);
        assert!((x[2] - 3.0).abs() < 1e-10);
    }
}
