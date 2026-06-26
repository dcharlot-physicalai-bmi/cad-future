//! `physical-brep` — B-rep geometry kernel for the Physical AI platform.
//!
//! This crate provides a half-edge boundary representation (B-Rep) data
//! structure with a full suite of modelling operations:
//!
//! - **Topology** ([`types`], [`solid`], [`query`]): vertices, edges, faces, and
//!   the [`Solid`] half-edge mesh that ties them together.
//! - **Geometry** ([`curve`], [`surface`]): [`Curve`] variants (line, arc,
//!   circle, NURBS) and [`Surface`] variants (plane, cylinder, sphere, torus,
//!   cone, NURBS).
//! - **Primitives** ([`builder`]): [`make_box`] and [`make_cylinder`].
//! - **Sweep-class operations** ([`extrude`], [`revolve`], [`loft`], [`sweep`]):
//!   turn 2D profiles into 3D solids.
//! - **Local operations** ([`fillet`], [`shell`], [`threads`]): edge blends,
//!   wall-offset hollowing, and cosmetic thread annotations.
//! - **Boolean CSG** ([`boolean`]): [`union`], [`subtract`], [`intersect`].
//! - **Patterning** ([`pattern`]): linear array, circular array, mirror.
//! - **2D profiles** ([`profile`]): closed loop of line/arc segments used as
//!   extrusion input.
//! - **Sheet metal** ([`sheet_metal`]): k-factor LUT, bend allowance, flat
//!   pattern, springback, relief cuts, and 3D fold.
//! - **Assembly** ([`assembly`]): rigid-body placement, mate constraints, and
//!   interference detection.

// ---------------------------------------------------------------------------
// Module declarations
// ---------------------------------------------------------------------------

pub mod types;
pub mod curve;
pub mod surface;
pub mod solid;
pub mod query;
pub mod profile;
pub mod builder;
pub mod boolean;
pub mod extrude;
pub mod revolve;
pub mod loft;
pub mod sweep;
pub mod fillet;
pub mod shell;
pub mod pattern;
pub mod sheet_metal;
pub mod assembly;
pub mod threads;

// ---------------------------------------------------------------------------
// Crate-root re-exports — the most commonly needed types and functions
// ---------------------------------------------------------------------------

// --- Topological IDs ---
pub use types::{VertexId, EdgeId, HalfEdgeId, FaceId};

// --- Topological entities ---
pub use types::{BRepVertex, HalfEdge, BRepEdge, BRepFace};

// --- Core solid ---
pub use solid::Solid;

// --- Geometry: curve ---
pub use curve::{Curve, bspline_basis, nurbs_curve_point, nurbs_uniform, perpendicular_frame};

// --- Geometry: surface ---
pub use surface::{Surface, nurbs_surface_point, nurbs_closest_point, intersect_surfaces,
                  fit_nurbs_through_points, nurbs_surface_uniform, uniform_knot_vector};

// --- 2D profile ---
pub use profile::{Profile, ProfileSegment};

// --- Primitive builders ---
pub use builder::{make_box, make_cylinder};

// --- Boolean operations ---
pub use boolean::{BooleanOp, boolean, union, subtract, intersect, boolean_with_splitting};
pub use boolean::{clip_polygon_by_plane, signed_volume, volume, surface_area, classify_point, Classification};
pub use boolean::{point_in_solid, point_in_solid_uncached, point_in_solid_with_accel,
                  SolidAccel, AccelCache, AccelCacheStats,
                  accel_cache_stats, accel_cache_clear};

// --- Extrude ---
pub use extrude::{extrude, extrude_z, extrude_symmetric};

// --- Revolve ---
pub use revolve::{revolve, revolve_full};

// --- Loft ---
pub use loft::{loft, loft_profiles, loft_nurbs, loft_nurbs_guided};

// --- Sweep ---
pub use sweep::{sweep, sweep_guided, frenet_frame, rotation_minimizing_frames, FrenetFrame, path_tangent};

// --- Fillet / chamfer ---
pub use fillet::{fillet, chamfer, rolling_ball_fillet, variable_radius_fillet,
                 EdgeConvexity, FilletError, classify_edge_convexity,
                 fillet_edge, fillet_edge_variable};

// --- Shell ---
pub use shell::shell;

// --- Pattern ---
pub use pattern::{linear_pattern, circular_pattern, mirror};

// --- Sheet metal ---
pub use sheet_metal::{
    KFactorEntry,
    K_FACTOR_TABLE,
    lookup_k_factor,
    bend_allowance,
    bend_deduction,
    outside_setback,
    Bend,
    Flange,
    SheetMetalPart,
    FlatPoint,
    FlatPattern,
    unfold,
    BendOp,
    optimize_bend_sequence,
    ReliefType,
    ReliefCut,
    compute_relief_cuts,
    FoldedSegment,
    fold_3d,
    springback_angle,
    check_min_bend_radius,
};

// --- Assembly ---
pub use assembly::{
    Placement,
    AssemblyPart,
    MateRef,
    MateType,
    Mate,
    Assembly,
    AssemblySolveResult,
};

// --- Threads ---
pub use threads::{
    ThreadStandard,
    ThreadType,
    ThreadClass,
    ThreadSpec,
    CosmeticThread,
    ThreadData,
    lookup_metric_coarse,
    lookup_metric_fine,
    suggest_thread_for_hole,
};
