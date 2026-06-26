//! Core B-Rep types — IDs and topological entities.

use glam::DVec3;
use serde::{Serialize, Deserialize};
use slotmap::new_key_type;

new_key_type! {
    /// Vertex identifier.
    pub struct VertexId;
    /// Half-edge identifier.
    pub struct HalfEdgeId;
    /// Edge identifier.
    pub struct EdgeId;
    /// Face identifier.
    pub struct FaceId;
}

/// A topological vertex — a point in 3D space.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BRepVertex {
    pub point: DVec3,
}

/// A half-edge — directed edge belonging to one face.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HalfEdge {
    /// Origin vertex.
    pub origin: VertexId,
    /// Parent edge.
    pub edge: EdgeId,
    /// Face this half-edge borders.
    pub face: FaceId,
    /// Next half-edge in the face loop.
    pub next: HalfEdgeId,
    /// Twin half-edge (on adjacent face).
    pub twin: Option<HalfEdgeId>,
}

/// A topological edge — pairs two half-edges.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BRepEdge {
    pub curve: crate::curve::Curve,
    pub half_edges: [HalfEdgeId; 2],
    /// Start vertex.
    pub v_start: VertexId,
    /// End vertex.
    pub v_end: VertexId,
}

/// A topological face — bounded by one or more loops of half-edges.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BRepFace {
    pub surface: crate::surface::Surface,
    /// Outer boundary loop (ordered half-edge IDs).
    pub outer_loop: Vec<HalfEdgeId>,
    /// Inner boundary loops (holes).
    pub holes: Vec<Vec<HalfEdgeId>>,
    /// Outward-facing normal hint (for orientation).
    pub normal_outward: bool,
}
