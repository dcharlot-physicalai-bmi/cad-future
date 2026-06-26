//! Mate constraint system — object-to-object spatial relationships.
//!
//! Provides operations for snapping, aligning, and constraining objects
//! relative to each other. Supports both immediate operations (move A onto B)
//! and persistent constraints (keep A flush against B).

use glam::{Mat4, Vec3};

// ── Mate operations (immediate, one-shot) ───────────────────────────────

/// The type of mate operation to perform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MateOp {
    /// Stack object A on top of object B (+Y).
    StackOnTop,
    /// Place A below B (-Y).
    StackBelow,
    /// Align centers along X axis.
    AlignX,
    /// Align centers along Y axis.
    AlignY,
    /// Align centers along Z axis.
    AlignZ,
    /// Make A flush against B's +X face.
    FlushPosX,
    /// Make A flush against B's -X face.
    FlushNegX,
    /// Make A flush against B's +Z face.
    FlushPosZ,
    /// Make A flush against B's -Z face.
    FlushNegZ,
    /// Concentric — align A's center with B's center (XZ), keep A's Y.
    Concentric,
    /// Offset — move A away from B by a given distance along the line between them.
    Offset(f32),
}

impl MateOp {
    pub fn label(self) -> &'static str {
        match self {
            Self::StackOnTop => "Stack on top",
            Self::StackBelow => "Stack below",
            Self::AlignX => "Align X",
            Self::AlignY => "Align Y",
            Self::AlignZ => "Align Z",
            Self::FlushPosX => "Flush +X",
            Self::FlushNegX => "Flush -X",
            Self::FlushPosZ => "Flush +Z",
            Self::FlushNegZ => "Flush -Z",
            Self::Concentric => "Concentric",
            Self::Offset(_) => "Offset",
        }
    }

    /// All basic mate operations (no offset — that requires a parameter).
    pub fn all_basic() -> &'static [MateOp] {
        &[
            MateOp::StackOnTop,
            MateOp::StackBelow,
            MateOp::AlignX,
            MateOp::AlignY,
            MateOp::AlignZ,
            MateOp::FlushPosX,
            MateOp::FlushNegX,
            MateOp::FlushPosZ,
            MateOp::FlushNegZ,
            MateOp::Concentric,
        ]
    }
}

/// Bounding box for an object (axis-aligned in local space).
#[derive(Clone, Copy, Debug)]
pub struct ObjBounds {
    pub center: Vec3,
    pub half_extents: Vec3,
}

impl ObjBounds {
    /// Extract bounds from a transform matrix (assumes unit-cube primitives scaled).
    pub fn from_transform(transform: Mat4) -> Self {
        let center = transform.col(3).truncate();
        let sx = transform.col(0).truncate().length();
        let sy = transform.col(1).truncate().length();
        let sz = transform.col(2).truncate().length();
        Self {
            center,
            half_extents: Vec3::new(sx * 0.5, sy * 0.5, sz * 0.5),
        }
    }

    pub fn min(&self) -> Vec3 {
        self.center - self.half_extents
    }

    pub fn max(&self) -> Vec3 {
        self.center + self.half_extents
    }

    pub fn top(&self) -> f32 {
        self.center.y + self.half_extents.y
    }

    pub fn bottom(&self) -> f32 {
        self.center.y - self.half_extents.y
    }
}

/// Compute a new transform for object A to mate it with object B.
///
/// Returns the new transform for A.
pub fn compute_mate(
    op: MateOp,
    a_transform: Mat4,
    b_transform: Mat4,
) -> Mat4 {
    let a = ObjBounds::from_transform(a_transform);
    let b = ObjBounds::from_transform(b_transform);

    // Extract A's scale and rotation (preserve them, only change position)
    let a_scale = Vec3::new(
        a_transform.col(0).truncate().length(),
        a_transform.col(1).truncate().length(),
        a_transform.col(2).truncate().length(),
    );

    let a_pos = a.center;

    let new_pos = match op {
        MateOp::StackOnTop => {
            // Place A on top of B: A's bottom = B's top
            let new_y = b.top() + a.half_extents.y;
            Vec3::new(b.center.x, new_y, b.center.z)
        }
        MateOp::StackBelow => {
            let new_y = b.bottom() - a.half_extents.y;
            Vec3::new(b.center.x, new_y, b.center.z)
        }
        MateOp::AlignX => {
            Vec3::new(b.center.x, a_pos.y, a_pos.z)
        }
        MateOp::AlignY => {
            Vec3::new(a_pos.x, b.center.y, a_pos.z)
        }
        MateOp::AlignZ => {
            Vec3::new(a_pos.x, a_pos.y, b.center.z)
        }
        MateOp::FlushPosX => {
            let new_x = b.max().x + a.half_extents.x;
            Vec3::new(new_x, a_pos.y, a_pos.z)
        }
        MateOp::FlushNegX => {
            let new_x = b.min().x - a.half_extents.x;
            Vec3::new(new_x, a_pos.y, a_pos.z)
        }
        MateOp::FlushPosZ => {
            let new_z = b.max().z + a.half_extents.z;
            Vec3::new(a_pos.x, a_pos.y, new_z)
        }
        MateOp::FlushNegZ => {
            let new_z = b.min().z - a.half_extents.z;
            Vec3::new(a_pos.x, a_pos.y, new_z)
        }
        MateOp::Concentric => {
            Vec3::new(b.center.x, a_pos.y, b.center.z)
        }
        MateOp::Offset(dist) => {
            let dir = (a_pos - b.center).normalize_or_zero();
            b.center + dir * dist
        }
    };

    Mat4::from_translation(new_pos) * Mat4::from_scale(a_scale)
}

// ── Persistent constraints ──────────────────────────────────────────────

/// A named constraint between two objects.
#[derive(Clone, Debug)]
pub struct MateConstraint {
    /// Human-readable name.
    pub name: String,
    /// Index of the first (moving) object.
    pub object_a: usize,
    /// Index of the reference (fixed) object.
    pub object_b: usize,
    /// The mate operation.
    pub op: MateOp,
    /// Whether this constraint is active.
    pub active: bool,
}

impl MateConstraint {
    pub fn new(name: &str, a: usize, b: usize, op: MateOp) -> Self {
        Self {
            name: name.to_string(),
            object_a: a,
            object_b: b,
            op,
            active: true,
        }
    }
}

/// Manages a list of mate constraints.
pub struct MateSystem {
    pub constraints: Vec<MateConstraint>,
}

impl MateSystem {
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }

    pub fn add(&mut self, constraint: MateConstraint) {
        self.constraints.push(constraint);
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.constraints.len() {
            self.constraints.remove(index);
        }
    }

    /// Solve all active constraints. Returns a list of (object_index, new_transform) updates.
    /// `get_transform` provides the current transform for any object index.
    pub fn solve<F>(&self, get_transform: F) -> Vec<(usize, Mat4)>
    where
        F: Fn(usize) -> Mat4,
    {
        let mut updates = Vec::new();
        for c in &self.constraints {
            if !c.active {
                continue;
            }
            let a_t = get_transform(c.object_a);
            let b_t = get_transform(c.object_b);
            let new_t = compute_mate(c.op, a_t, b_t);
            updates.push((c.object_a, new_t));
        }
        updates
    }
}

impl Default for MateSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube_at(x: f32, y: f32, z: f32) -> Mat4 {
        Mat4::from_translation(Vec3::new(x, y, z))
    }

    fn scaled_cube_at(x: f32, y: f32, z: f32, s: f32) -> Mat4 {
        Mat4::from_translation(Vec3::new(x, y, z)) * Mat4::from_scale(Vec3::splat(s))
    }

    #[test]
    fn stack_on_top() {
        let a = cube_at(0.0, 0.5, 0.0); // unit cube centered at (0, 0.5, 0)
        let b = cube_at(0.0, 0.5, 0.0);
        let result = compute_mate(MateOp::StackOnTop, a, b);
        let pos = result.col(3).truncate();
        // A should be at y=1.5 (B top at 1.0, A half=0.5)
        assert!((pos.y - 1.5).abs() < 0.01, "pos.y = {}", pos.y);
        assert!((pos.x).abs() < 0.01);
    }

    #[test]
    fn stack_below() {
        let a = cube_at(0.0, 2.0, 0.0);
        let b = cube_at(0.0, 0.5, 0.0);
        let result = compute_mate(MateOp::StackBelow, a, b);
        let pos = result.col(3).truncate();
        // B bottom at 0.0, A half=0.5 → A center at -0.5
        assert!((pos.y + 0.5).abs() < 0.01, "pos.y = {}", pos.y);
    }

    #[test]
    fn align_x() {
        let a = cube_at(5.0, 1.0, 3.0);
        let b = cube_at(0.0, 0.5, 0.0);
        let result = compute_mate(MateOp::AlignX, a, b);
        let pos = result.col(3).truncate();
        assert!((pos.x - 0.0).abs() < 0.01); // aligned to B's X
        assert!((pos.y - 1.0).abs() < 0.01); // Y unchanged
        assert!((pos.z - 3.0).abs() < 0.01); // Z unchanged
    }

    #[test]
    fn flush_pos_x() {
        let a = cube_at(0.0, 0.5, 0.0);
        let b = cube_at(0.0, 0.5, 0.0);
        let result = compute_mate(MateOp::FlushPosX, a, b);
        let pos = result.col(3).truncate();
        // B max x = 0.5, A half = 0.5 → A center = 1.0
        assert!((pos.x - 1.0).abs() < 0.01, "pos.x = {}", pos.x);
    }

    #[test]
    fn concentric() {
        let a = cube_at(5.0, 2.0, 3.0);
        let b = cube_at(1.0, 0.5, 1.0);
        let result = compute_mate(MateOp::Concentric, a, b);
        let pos = result.col(3).truncate();
        assert!((pos.x - 1.0).abs() < 0.01);
        assert!((pos.y - 2.0).abs() < 0.01); // Y preserved
        assert!((pos.z - 1.0).abs() < 0.01);
    }

    #[test]
    fn preserves_scale() {
        let a = scaled_cube_at(0.0, 1.0, 0.0, 2.0);
        let b = cube_at(0.0, 0.5, 0.0);
        let result = compute_mate(MateOp::StackOnTop, a, b);
        let sx = result.col(0).truncate().length();
        assert!((sx - 2.0).abs() < 0.01, "scale preserved");
    }

    #[test]
    fn mate_system_solve() {
        let sys = MateSystem {
            constraints: vec![
                MateConstraint::new("test", 0, 1, MateOp::AlignX),
            ],
        };
        let transforms = [
            cube_at(5.0, 0.5, 0.0),
            cube_at(0.0, 0.5, 0.0),
        ];
        let updates = sys.solve(|i| transforms[i]);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].0, 0);
        let pos = updates[0].1.col(3).truncate();
        assert!((pos.x).abs() < 0.01);
    }
}
