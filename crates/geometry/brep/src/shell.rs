//! Shell (hollow) operation for B-Rep solids.
//!
//! Hollows out a solid by offsetting all faces inward by a wall thickness,
//! optionally removing one or more faces to create openings.

use std::collections::{HashMap, HashSet};

use glam::DVec3;

use crate::solid::Solid;
use crate::surface::Surface;
use crate::types::VertexId;

/// Shell a solid: create a hollow version with given wall thickness.
/// `open_faces` are face indices (0-based into `face_ids()`) to remove (creating openings).
///
/// The algorithm works per-vertex: for each vertex shared by multiple faces,
/// an average inward normal is computed and the vertex is offset along that
/// direction by `thickness`. The result has:
/// - **Outer faces** — copies of the original faces, excluding open faces.
/// - **Inner faces** — offset copies of every non-open face with reversed winding.
/// - **Rim faces** — quads connecting outer and inner edges at open-face boundaries.
pub fn shell(solid: &Solid, thickness: f64, open_faces: &[usize]) -> Solid {
    let face_ids = solid.face_ids();
    let open_set: HashSet<usize> = open_faces.iter().copied().collect();

    // --- Step 1: Collect per-face vertex data and compute per-vertex average inward normal ---

    // Map from VertexId to list of outward normals from faces that use it.
    let mut vertex_normals: HashMap<VertexId, Vec<DVec3>> = HashMap::new();

    // Per-face ordered vertex list (for easy access later).
    let mut face_verts: Vec<Vec<VertexId>> = Vec::with_capacity(face_ids.len());

    for &fid in &face_ids {
        let verts: Vec<VertexId> = solid.faces[fid]
            .outer_loop
            .iter()
            .map(|he| solid.half_edges[*he].origin)
            .collect();

        // Compute the outward face normal from the surface.
        let centroid = face_centroid(solid, &verts);
        let normal = solid.faces[fid].surface.normal_at(centroid);

        for &vid in &verts {
            vertex_normals.entry(vid).or_default().push(normal);
        }

        face_verts.push(verts);
    }

    // Average inward normal per vertex.
    let vertex_offset: HashMap<VertexId, DVec3> = vertex_normals
        .iter()
        .map(|(&vid, normals)| {
            let avg: DVec3 = normals.iter().copied().sum::<DVec3>() / normals.len() as f64;
            let inward = if avg.length_squared() > 1e-20 {
                -avg.normalize()
            } else {
                DVec3::ZERO
            };
            (vid, inward * thickness)
        })
        .collect();

    // --- Step 2: Build the new solid ---

    let mut result = Solid::new();

    // Create outer and inner vertex copies.
    let mut outer_map: HashMap<VertexId, VertexId> = HashMap::new();
    let mut inner_map: HashMap<VertexId, VertexId> = HashMap::new();

    for (&vid, _) in &vertex_offset {
        let pt = solid.vertices[vid].point;
        let outer_id = result.add_vertex(pt);
        let inner_id = result.add_vertex(pt + vertex_offset[&vid]);
        outer_map.insert(vid, outer_id);
        inner_map.insert(vid, inner_id);
    }

    // --- Step 3: Add outer faces (non-open only) ---

    for (idx, &fid) in face_ids.iter().enumerate() {
        if open_set.contains(&idx) {
            continue;
        }
        let verts = &face_verts[idx];
        let mapped: Vec<VertexId> = verts.iter().map(|v| outer_map[v]).collect();
        let surface = solid.faces[fid].surface.clone();
        result.add_face_from_vertices(surface, &mapped, true);
    }

    // --- Step 4: Add inner faces (non-open, reversed winding) ---

    for (idx, &fid) in face_ids.iter().enumerate() {
        if open_set.contains(&idx) {
            continue;
        }
        let verts = &face_verts[idx];
        let reversed: Vec<VertexId> = verts.iter().rev().map(|v| inner_map[v]).collect();
        let surface = solid.faces[fid].surface.flipped();

        // Offset the surface origin inward for planes.
        let inner_surface = offset_surface(&surface, thickness);
        result.add_face_from_vertices(inner_surface, &reversed, true);
    }

    // --- Step 5: Add rim faces at open-face boundaries ---
    //
    // For each edge of an open face that borders a non-open face, we need a
    // quad connecting the outer edge to the inner edge.
    //
    // An edge on an open face's boundary is identified by the pair of consecutive
    // vertices in the open face's loop. The rim quad connects:
    //   outer[a] -> outer[b] -> inner[b] -> inner[a]

    for &open_idx in &open_set {
        let verts = &face_verts[open_idx];
        let n = verts.len();
        for i in 0..n {
            let va = verts[i];
            let vb = verts[(i + 1) % n];

            // Compute a rim face normal: perpendicular to edge direction and
            // to the average of the outer and inner offset direction.
            let pa = solid.vertices[va].point;
            let pb = solid.vertices[vb].point;
            let mid = (pa + pb) * 0.5;

            // The outward normal of the open face gives us the rim normal direction.
            let open_fid = face_ids[open_idx];
            let face_normal = solid.faces[open_fid].surface.normal_at(mid);
            let rim_normal = face_normal;

            let rim_surface = Surface::plane(mid, rim_normal);

            // Quad: outer_a -> outer_b -> inner_b -> inner_a
            let quad = [
                outer_map[&va],
                outer_map[&vb],
                inner_map[&vb],
                inner_map[&va],
            ];
            result.add_face_from_vertices(rim_surface, &quad, true);
        }
    }

    result.link_twins();
    result
}

/// Compute the centroid of a set of vertices.
fn face_centroid(solid: &Solid, verts: &[VertexId]) -> DVec3 {
    let sum: DVec3 = verts
        .iter()
        .map(|&v| solid.vertices[v].point)
        .sum();
    sum / verts.len() as f64
}

/// Offset a surface origin inward (for planes, shift origin along the flipped normal).
/// For other surface types, return as-is (the flipped normal is already set).
fn offset_surface(surface: &Surface, thickness: f64) -> Surface {
    match surface {
        Surface::Plane { origin, normal } => {
            // The surface has already been flipped, so normal points inward.
            // Offset origin along the (original outward) direction, which is -normal here.
            Surface::plane(*origin + *normal * thickness, *normal)
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::make_box;

    /// Helper: approximate volume of a solid via its bounding box.
    fn bbox_volume(solid: &Solid) -> f64 {
        let (min, max) = solid.bounding_box();
        let d = max - min;
        d.x * d.y * d.z
    }

    #[test]
    fn shell_box_one_open_face() {
        // 10x10x10 box, shell with thickness 1, remove the top face (index 4 = +Y).
        let b = make_box(10.0, 10.0, 10.0);
        assert_eq!(b.face_count(), 6);

        let shelled = shell(&b, 1.0, &[4]);

        // Expected topology:
        //   outer faces: 5  (6 - 1 open)
        //   inner faces: 5  (same, offset inward, reversed)
        //   rim faces:   4  (one quad per edge of the removed top face)
        //   total faces = 14
        assert_eq!(shelled.face_count(), 14);

        // Euler characteristic: should be 2 for a valid closed shell.
        assert!(
            shelled.is_valid_shell(),
            "Euler V-E+F = {} (expected 2)",
            shelled.euler_characteristic()
        );
    }

    #[test]
    fn shell_box_no_open_faces() {
        // Fully enclosed double wall (no openings).
        let b = make_box(10.0, 10.0, 10.0);
        let shelled = shell(&b, 1.0, &[]);

        // 6 outer + 6 inner = 12 faces, 0 rim faces.
        assert_eq!(shelled.face_count(), 12);

        // A fully enclosed double wall is two separate closed shells,
        // so Euler = 2 + 2 = 4 for disconnected shells.
        // However since link_twins only connects shared vertex pairs
        // and outer/inner share no vertices, they remain topologically independent.
        // Each shell individually has Euler 2; combined V-E+F = 4.
        assert_eq!(shelled.euler_characteristic(), 4);
    }

    #[test]
    fn shell_preserves_outer_dimensions() {
        let b = make_box(20.0, 10.0, 30.0);
        let (orig_min, orig_max) = b.bounding_box();

        let shelled = shell(&b, 2.0, &[0]);
        let (shell_min, shell_max) = shelled.bounding_box();

        // Outer bounding box should be identical.
        assert!(
            (shell_min - orig_min).length() < 1e-10,
            "Min changed: {:?} vs {:?}",
            shell_min,
            orig_min
        );
        assert!(
            (shell_max - orig_max).length() < 1e-10,
            "Max changed: {:?} vs {:?}",
            shell_max,
            orig_max
        );
    }

    #[test]
    fn shell_reduces_volume() {
        // The inner bounding box should be strictly smaller than the outer.
        let b = make_box(10.0, 10.0, 10.0);
        let shelled = shell(&b, 1.0, &[4]);

        // Collect all inner vertices (those created via offset).
        // They should all be strictly inside the original bounding box.
        let (orig_min, orig_max) = b.bounding_box();
        let eps = 0.5; // inner vertices should be at least ~1.0 inside on faces

        // Check that at least some vertices are inside the original bbox.
        let inner_count = shelled
            .vertices
            .values()
            .filter(|v| {
                v.point.x > orig_min.x + eps
                    && v.point.x < orig_max.x - eps
                    && v.point.y > orig_min.y + eps
                    && v.point.y < orig_max.y - eps
                    && v.point.z > orig_min.z + eps
                    && v.point.z < orig_max.z - eps
            })
            .count();

        assert!(
            inner_count > 0,
            "Expected inner vertices strictly inside the original bounding box"
        );

        // The outer bounding box volume should be the same as original.
        let outer_vol = bbox_volume(&b);
        let shelled_vol = bbox_volume(&shelled);
        assert!(
            (shelled_vol - outer_vol).abs() < 1e-6,
            "Outer bounding box volume changed"
        );
    }
}
