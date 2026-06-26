//! `physical-sketch` — 2D parametric sketch constraint solver.
//!
//! A sketch is a collection of geometric [`SketchEntity`] primitives (points,
//! lines, circles, arcs) paired with [`Constraint`]s that express relationships
//! between them (coincident endpoints, fixed positions, distances, angles,
//! tangency, etc.).  The Newton-Raphson solver in [`solver`] drives entity
//! parameters until all constraint residuals converge to zero.  Once solved,
//! [`profile_extract`] can walk connected line segments and extract closed loops
//! ready for extrusion by `physical-brep`.

pub mod entity;
pub mod constraint;
pub mod sketch;
pub mod solver;
pub mod profile_extract;

// --- Entity types ---
pub use entity::{EntityId, PointRef, SketchEntity};

// --- Constraint types ---
pub use constraint::{ConstraintId, Constraint};

// --- Sketch container ---
pub use sketch::Sketch;

// --- Solver ---
pub use solver::{solve, solve_with_diagnostics, SolveResult, SolverDiagnostics};
pub use solver::{diagnose, suggest_constraints, SketchStatus, SuggestedConstraint};

// --- Profile extraction ---
pub use profile_extract::{extract_profiles, to_brep_profile, ExtractedSegment};
