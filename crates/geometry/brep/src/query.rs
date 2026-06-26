//! Adjacency queries on B-Rep topology.

use crate::types::*;
use crate::solid::Solid;

impl Solid {
    /// Get the two faces adjacent to an edge.
    pub fn faces_of_edge(&self, edge_id: EdgeId) -> Vec<FaceId> {
        let edge = &self.edges[edge_id];
        let mut faces = Vec::with_capacity(2);
        faces.push(self.half_edges[edge.half_edges[0]].face);
        if edge.half_edges[1] != HalfEdgeId::default() {
            faces.push(self.half_edges[edge.half_edges[1]].face);
        }
        faces
    }

    /// Get the two vertices of an edge.
    pub fn vertices_of_edge(&self, edge_id: EdgeId) -> (VertexId, VertexId) {
        let edge = &self.edges[edge_id];
        (edge.v_start, edge.v_end)
    }

    /// Get all edges of a face.
    pub fn edges_of_face(&self, face_id: FaceId) -> Vec<EdgeId> {
        let face = &self.faces[face_id];
        face.outer_loop.iter().map(|he_id| self.half_edges[*he_id].edge).collect()
    }

    /// Get all faces sharing a vertex.
    pub fn faces_of_vertex(&self, vertex_id: VertexId) -> Vec<FaceId> {
        let mut result = Vec::new();
        for (fid, face) in &self.faces {
            for he_id in &face.outer_loop {
                if self.half_edges[*he_id].origin == vertex_id {
                    result.push(fid);
                    break;
                }
            }
        }
        result
    }

    /// Get all edges connected to a vertex.
    pub fn edges_of_vertex(&self, vertex_id: VertexId) -> Vec<EdgeId> {
        let mut result = Vec::new();
        for (eid, edge) in &self.edges {
            if edge.v_start == vertex_id || edge.v_end == vertex_id {
                result.push(eid);
            }
        }
        result
    }

    /// Get the midpoint of an edge (from its curve).
    pub fn edge_midpoint(&self, edge_id: EdgeId) -> glam::DVec3 {
        self.edges[edge_id].curve.midpoint()
    }

    /// Angle between two faces at a shared edge (dihedral angle).
    /// Returns angle in radians, < PI for convex, > PI for concave.
    pub fn dihedral_angle(&self, edge_id: EdgeId) -> Option<f64> {
        let faces = self.faces_of_edge(edge_id);
        if faces.len() != 2 { return None; }

        let edge = &self.edges[edge_id];
        let mid = edge.curve.midpoint();

        let n1 = self.faces[faces[0]].surface.normal_at(mid);
        let n2 = self.faces[faces[1]].surface.normal_at(mid);

        let cos_a = n1.dot(n2).clamp(-1.0, 1.0);
        Some(cos_a.acos())
    }
}

#[cfg(test)]
mod tests {
    // Tested via builder::make_box in builder.rs
}
