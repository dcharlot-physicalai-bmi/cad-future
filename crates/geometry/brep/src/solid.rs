//! Solid — the central B-Rep data structure.

use glam::DVec3;
use serde::{Serialize, Deserialize};
use slotmap::SlotMap;

use crate::types::*;
use crate::curve::Curve;
use crate::surface::Surface;

/// A B-Rep solid — manifold closed shell.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Solid {
    pub vertices: SlotMap<VertexId, BRepVertex>,
    pub half_edges: SlotMap<HalfEdgeId, HalfEdge>,
    pub edges: SlotMap<EdgeId, BRepEdge>,
    pub faces: SlotMap<FaceId, BRepFace>,
}

impl Solid {
    pub fn new() -> Self {
        Self {
            vertices: SlotMap::with_key(),
            half_edges: SlotMap::with_key(),
            edges: SlotMap::with_key(),
            faces: SlotMap::with_key(),
        }
    }

    /// Add a vertex, return its ID.
    pub fn add_vertex(&mut self, point: DVec3) -> VertexId {
        self.vertices.insert(BRepVertex { point })
    }

    /// Add a face with its surface and outer loop defined by ordered vertex IDs.
    /// Creates edges and half-edges automatically.
    pub fn add_face_from_vertices(
        &mut self,
        surface: Surface,
        vertex_ids: &[VertexId],
        outward: bool,
    ) -> FaceId {
        let face_id = self.faces.insert(BRepFace {
            surface,
            outer_loop: Vec::new(),
            holes: Vec::new(),
            normal_outward: outward,
        });

        let n = vertex_ids.len();
        let mut he_ids = Vec::with_capacity(n);

        for i in 0..n {
            let v_start = vertex_ids[i];
            let v_end = vertex_ids[(i + 1) % n];
            let p_start = self.vertices[v_start].point;
            let p_end = self.vertices[v_end].point;

            let curve = Curve::line(p_start, p_end);

            // Create edge placeholder — we'll fill half_edges after
            let edge_id = self.edges.insert(BRepEdge {
                curve,
                half_edges: [HalfEdgeId::default(), HalfEdgeId::default()],
                v_start,
                v_end,
            });

            let he_id = self.half_edges.insert(HalfEdge {
                origin: v_start,
                edge: edge_id,
                face: face_id,
                next: HalfEdgeId::default(), // filled below
                twin: None,
            });

            self.edges[edge_id].half_edges[0] = he_id;
            he_ids.push(he_id);
        }

        // Link next pointers in the loop
        for i in 0..n {
            self.half_edges[he_ids[i]].next = he_ids[(i + 1) % n];
        }

        self.faces[face_id].outer_loop = he_ids;
        face_id
    }

    /// Link twin half-edges between two faces that share an edge.
    /// Finds matching edges by vertex pairs.
    pub fn link_twins(&mut self) {
        let he_keys: Vec<HalfEdgeId> = self.half_edges.keys().collect();
        let n = he_keys.len();
        for i in 0..n {
            if self.half_edges[he_keys[i]].twin.is_some() { continue; }
            let he_i = &self.half_edges[he_keys[i]];
            let v_start_i = he_i.origin;
            let next_i = he_i.next;
            let v_end_i = self.half_edges[next_i].origin;

            for j in (i + 1)..n {
                if self.half_edges[he_keys[j]].twin.is_some() { continue; }
                let he_j = &self.half_edges[he_keys[j]];
                let v_start_j = he_j.origin;
                let next_j = he_j.next;
                let v_end_j = self.half_edges[next_j].origin;

                // Twin: opposite direction
                if v_start_i == v_end_j && v_end_i == v_start_j {
                    self.half_edges[he_keys[i]].twin = Some(he_keys[j]);
                    self.half_edges[he_keys[j]].twin = Some(he_keys[i]);

                    // Share the edge
                    let edge_i = self.half_edges[he_keys[i]].edge;
                    self.edges[edge_i].half_edges[1] = he_keys[j];
                    // Remove the duplicate edge from he_j
                    let old_edge = self.half_edges[he_keys[j]].edge;
                    self.half_edges[he_keys[j]].edge = edge_i;
                    self.edges.remove(old_edge);
                    break;
                }
            }
        }
    }

    /// Euler characteristic V - E + F (should be 2 for a valid closed shell).
    pub fn euler_characteristic(&self) -> i32 {
        self.vertices.len() as i32 - self.edges.len() as i32 + self.faces.len() as i32
    }

    /// Validate: check Euler characteristic == 2.
    pub fn is_valid_shell(&self) -> bool {
        self.euler_characteristic() == 2
    }

    /// Get all edge IDs.
    pub fn edge_ids(&self) -> Vec<EdgeId> {
        self.edges.keys().collect()
    }

    /// Get all face IDs.
    pub fn face_ids(&self) -> Vec<FaceId> {
        self.faces.keys().collect()
    }

    /// Vertex count.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Edge count.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Face count.
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Get the 3D points of a face's outer loop.
    pub fn face_vertices(&self, face_id: FaceId) -> Vec<DVec3> {
        let face = &self.faces[face_id];
        face.outer_loop.iter().map(|he_id| {
            let he = &self.half_edges[*he_id];
            self.vertices[he.origin].point
        }).collect()
    }

    /// Compute bounding box: (min, max).
    pub fn bounding_box(&self) -> (DVec3, DVec3) {
        let mut min = DVec3::splat(f64::MAX);
        let mut max = DVec3::splat(f64::MIN);
        for (_, v) in &self.vertices {
            min = min.min(v.point);
            max = max.max(v.point);
        }
        (min, max)
    }
}

impl Default for Solid {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_solid() {
        let s = Solid::new();
        assert_eq!(s.vertex_count(), 0);
        assert_eq!(s.edge_count(), 0);
        assert_eq!(s.face_count(), 0);
    }
}
