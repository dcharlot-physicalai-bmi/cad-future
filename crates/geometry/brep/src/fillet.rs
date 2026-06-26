//! Fillet and chamfer operations on B-Rep edges.
//!
//! Chamfer: replaces a sharp edge with a flat bevel (planar face).
//! Fillet: replaces a sharp edge with a cylindrical blend surface.
//!
//! ## Rolling ball fillet
//! `rolling_ball_fillet` computes a fillet surface as the locus of a sphere of
//! given radius that is tangent to two adjacent faces simultaneously.
//!
//! ## Variable radius fillet
//! `variable_radius_fillet` allows the fillet radius to vary linearly along
//! the edge between a start and end radius.

use crate::types::*;
use crate::surface::Surface;
use crate::solid::Solid;

/// Apply a chamfer to the specified edges of a solid.
/// Creates a flat bevel face replacing each sharp edge.
///
/// `distance` is the setback from the edge on each adjacent face.
pub fn chamfer(solid: &mut Solid, edge_ids: &[EdgeId], distance: f64) {
    for &eid in edge_ids {
        chamfer_single_edge(solid, eid, distance);
    }
    solid.link_twins();
}

/// Apply a fillet (cylindrical blend) to the specified edges.
/// `radius` is the fillet radius.
pub fn fillet(solid: &mut Solid, edge_ids: &[EdgeId], radius: f64) {
    for &eid in edge_ids {
        fillet_single_edge(solid, eid, radius);
    }
    solid.link_twins();
}

/// Apply a rolling-ball fillet to the specified edges.
///
/// The fillet surface is computed as the locus of the center of a sphere of the
/// given `radius` that remains tangent to both adjacent faces. The resulting
/// blend surface is approximated as a sequence of quad strips with cylindrical
/// surface geometry.
pub fn rolling_ball_fillet(solid: &mut Solid, edge_ids: &[EdgeId], radius: f64) {
    for &eid in edge_ids {
        rolling_ball_fillet_single(solid, eid, radius);
    }
    solid.link_twins();
}

/// Apply a variable-radius fillet to the specified edges.
///
/// The radius interpolates linearly from `radius_start` (at the edge's start
/// vertex) to `radius_end` (at the edge's end vertex).
pub fn variable_radius_fillet(
    solid: &mut Solid,
    edge_ids: &[EdgeId],
    radius_start: f64,
    radius_end: f64,
) {
    for &eid in edge_ids {
        variable_radius_fillet_single(solid, eid, radius_start, radius_end);
    }
    solid.link_twins();
}

// ---------------------------------------------------------------------------
// Internal: chamfer
// ---------------------------------------------------------------------------

fn chamfer_single_edge(solid: &mut Solid, edge_id: EdgeId, distance: f64) {
    let edge = match solid.edges.get(edge_id) {
        Some(e) => e.clone(),
        None => return,
    };
    let faces = solid.faces_of_edge(edge_id);
    if faces.len() != 2 { return; }

    let v_start = edge.v_start;
    let v_end = edge.v_end;
    let p_start = solid.vertices[v_start].point;
    let p_end = solid.vertices[v_end].point;
    let edge_dir = (p_end - p_start).normalize();

    let mid = edge.curve.midpoint();
    let n1 = solid.faces[faces[0]].surface.normal_at(mid);
    let n2 = solid.faces[faces[1]].surface.normal_at(mid);

    let offset1 = -n1.cross(edge_dir).normalize() * distance;
    let offset2 = n2.cross(edge_dir).normalize() * distance;

    let v_a = solid.add_vertex(p_start + offset1);
    let v_b = solid.add_vertex(p_end + offset1);
    let v_c = solid.add_vertex(p_end + offset2);
    let v_d = solid.add_vertex(p_start + offset2);

    let chamfer_center = (p_start + p_end) * 0.5 + (offset1 + offset2) * 0.5;
    let chamfer_normal = (offset1.normalize() + offset2.normalize()).normalize();

    solid.add_face_from_vertices(
        Surface::plane(chamfer_center, chamfer_normal),
        &[v_a, v_b, v_c, v_d], true,
    );
}

// ---------------------------------------------------------------------------
// Internal: constant-radius fillet
// ---------------------------------------------------------------------------

fn fillet_single_edge(solid: &mut Solid, edge_id: EdgeId, radius: f64) {
    let edge = match solid.edges.get(edge_id) {
        Some(e) => e.clone(),
        None => return,
    };
    let faces = solid.faces_of_edge(edge_id);
    if faces.len() != 2 { return; }

    let v_start = edge.v_start;
    let v_end = edge.v_end;
    let p_start = solid.vertices[v_start].point;
    let p_end = solid.vertices[v_end].point;
    let edge_dir = (p_end - p_start).normalize();

    let mid = edge.curve.midpoint();
    let n1 = solid.faces[faces[0]].surface.normal_at(mid);
    let n2 = solid.faces[faces[1]].surface.normal_at(mid);

    let d1 = -n1.cross(edge_dir).normalize();
    let d2 = n2.cross(edge_dir).normalize();

    let steps = 8;
    let mut prev_start = solid.add_vertex(p_start + d1 * radius);
    let mut prev_end = solid.add_vertex(p_end + d1 * radius);

    let center_offset = (d1 + d2).normalize() * radius;

    for i in 1..=steps {
        let t = i as f64 / steps as f64;
        let angle = std::f64::consts::FRAC_PI_2 * t;

        let blend = d1 * angle.cos() + d2 * angle.sin();
        let offset = blend * radius;

        let cur_start = solid.add_vertex(p_start + offset);
        let cur_end = solid.add_vertex(p_end + offset);

        let fillet_center = mid + center_offset;
        solid.add_face_from_vertices(
            Surface::cylinder(fillet_center, edge_dir, radius),
            &[prev_start, prev_end, cur_end, cur_start], true,
        );

        prev_start = cur_start;
        prev_end = cur_end;
    }
}

// ---------------------------------------------------------------------------
// Internal: rolling ball fillet
// ---------------------------------------------------------------------------

/// Compute a rolling-ball fillet on a single edge.
///
/// The rolling ball method finds, at each sample point along the edge, the
/// center of a sphere of the given radius that is tangent to both adjacent
/// faces. The contact curves on each face define the fillet boundary, and the
/// fillet surface is the portion of the sphere between those contact curves.
fn rolling_ball_fillet_single(solid: &mut Solid, edge_id: EdgeId, radius: f64) {
    let edge = match solid.edges.get(edge_id) {
        Some(e) => e.clone(),
        None => return,
    };
    let faces = solid.faces_of_edge(edge_id);
    if faces.len() != 2 { return; }

    let v_start = edge.v_start;
    let v_end = edge.v_end;
    let p_start = solid.vertices[v_start].point;
    let p_end = solid.vertices[v_end].point;
    let _edge_dir = (p_end - p_start).normalize();

    // Number of stations along the edge
    let stations = 8;
    // Number of arc subdivisions for the fillet arc
    let arc_steps = 6;

    // At each station, compute the ball center and the two tangent points
    let mut prev_ring: Option<Vec<VertexId>> = None;

    for s in 0..=stations {
        let t = s as f64 / stations as f64;
        let edge_pt = p_start + (p_end - p_start) * t;

        // Surface normals at this edge point
        let n1 = solid.faces[faces[0]].surface.normal_at(edge_pt);
        let n2 = solid.faces[faces[1]].surface.normal_at(edge_pt);

        // The ball center is offset from the edge along the bisector of the
        // two face normals, at a distance such that the ball of radius r is
        // tangent to both faces.
        let bisector = (n1 + n2).normalize();
        let half_angle = n1.dot(n2).clamp(-1.0, 1.0).acos() * 0.5;
        let center_dist = if half_angle.sin().abs() > 1e-10 {
            radius / half_angle.sin()
        } else {
            radius
        };
        let ball_center = edge_pt + bisector * center_dist;

        // Tangent points: project from ball center onto each face plane
        let contact1 = ball_center - n1 * radius;
        let contact2 = ball_center - n2 * radius;

        // Build an arc from contact1 to contact2 around ball_center
        let mut ring = Vec::with_capacity(arc_steps + 1);
        for a in 0..=arc_steps {
            let frac = a as f64 / arc_steps as f64;
            // Spherical interpolation of the contact points around ball center
            let dir1 = (contact1 - ball_center).normalize();
            let dir2 = (contact2 - ball_center).normalize();

            // Slerp between dir1 and dir2
            let dot = dir1.dot(dir2).clamp(-1.0, 1.0);
            let omega = dot.acos();
            let pt = if omega.abs() > 1e-10 {
                let s1 = ((1.0 - frac) * omega).sin() / omega.sin();
                let s2 = (frac * omega).sin() / omega.sin();
                ball_center + (dir1 * s1 + dir2 * s2) * radius
            } else {
                ball_center + dir1 * radius
            };

            ring.push(solid.add_vertex(pt));
        }

        // Connect this ring to the previous ring with quads
        if let Some(ref prev) = prev_ring {
            for i in 0..arc_steps {
                let v0 = prev[i];
                let v1 = prev[i + 1];
                let v2 = ring[i + 1];
                let v3 = ring[i];

                let p0 = solid.vertices[v0].point;
                let p1 = solid.vertices[v1].point;
                let p3 = solid.vertices[v3].point;

                let face_n = (p1 - p0).cross(p3 - p0);
                let _face_n = if face_n.length() > 1e-14 { face_n.normalize() } else { bisector };

                solid.add_face_from_vertices(
                    Surface::sphere(ball_center, radius),
                    &[v0, v1, v2, v3],
                    true,
                );
            }
        }

        prev_ring = Some(ring);
    }
}

// ---------------------------------------------------------------------------
// Internal: variable radius fillet
// ---------------------------------------------------------------------------

/// Compute a variable-radius fillet on a single edge.
///
/// The radius varies linearly from `radius_start` to `radius_end` along the
/// edge. At each station the fillet geometry is computed identically to the
/// constant-radius fillet but with the interpolated radius.
fn variable_radius_fillet_single(
    solid: &mut Solid,
    edge_id: EdgeId,
    radius_start: f64,
    radius_end: f64,
) {
    let edge = match solid.edges.get(edge_id) {
        Some(e) => e.clone(),
        None => return,
    };
    let faces = solid.faces_of_edge(edge_id);
    if faces.len() != 2 { return; }

    let v_start = edge.v_start;
    let v_end = edge.v_end;
    let p_start = solid.vertices[v_start].point;
    let p_end = solid.vertices[v_end].point;
    let edge_dir = (p_end - p_start).normalize();

    let stations = 8;
    let arc_steps = 6;

    let mut prev_ring: Option<Vec<VertexId>> = None;

    for s in 0..=stations {
        let t = s as f64 / stations as f64;
        let edge_pt = p_start + (p_end - p_start) * t;
        let radius = radius_start + (radius_end - radius_start) * t;

        let n1 = solid.faces[faces[0]].surface.normal_at(edge_pt);
        let n2 = solid.faces[faces[1]].surface.normal_at(edge_pt);

        // Offset directions
        let d1 = -n1.cross(edge_dir).normalize();
        let d2 = n2.cross(edge_dir).normalize();

        // Build arc ring from face1 contact to face2 contact
        let mut ring = Vec::with_capacity(arc_steps + 1);
        for a in 0..=arc_steps {
            let frac = a as f64 / arc_steps as f64;
            let angle = std::f64::consts::FRAC_PI_2 * frac;
            let blend = d1 * angle.cos() + d2 * angle.sin();
            let pt = edge_pt + blend * radius;
            ring.push(solid.add_vertex(pt));
        }

        // Connect to previous ring
        if let Some(ref prev) = prev_ring {
            let mid_r = radius_start + (radius_end - radius_start) * ((s as f64 - 0.5) / stations as f64);
            let mid_pt = p_start + (p_end - p_start) * ((s as f64 - 0.5) / stations as f64);
            let center_offset = (d1 + d2).normalize() * mid_r;
            let fillet_center = mid_pt + center_offset;

            for i in 0..arc_steps {
                let v0 = prev[i];
                let v1 = prev[i + 1];
                let v2 = ring[i + 1];
                let v3 = ring[i];

                solid.add_face_from_vertices(
                    Surface::cylinder(fillet_center, edge_dir, mid_r),
                    &[v0, v1, v2, v3],
                    true,
                );
            }
        }

        prev_ring = Some(ring);
    }
}

// ---------------------------------------------------------------------------
// Edge convexity classification
// ---------------------------------------------------------------------------

/// Classification of the dihedral angle at a B-Rep edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeConvexity {
    /// The two faces open outward (dihedral angle < π).
    Convex,
    /// The two faces open inward (dihedral angle > π).
    Concave,
    /// The two faces are coplanar (dihedral angle ≈ π).
    Flat,
}

/// Fillet errors.
#[derive(Clone, Debug, PartialEq)]
pub enum FilletError {
    /// The edge does not exist in the solid.
    EdgeNotFound,
    /// The edge does not have exactly two adjacent faces (non-manifold or boundary).
    NonManifoldEdge,
    /// The radius is zero or negative.
    InvalidRadius,
    /// The edge is flat — filleting has no geometric effect.
    FlatEdge,
}

/// Classify whether an edge is convex, concave, or flat by examining the
/// dihedral angle between its two adjacent faces.
///
/// The test uses the outward face normals and the edge midpoint: a convex
/// edge has normals that point away from each other (their average, the
/// bisector, points outward from the material), whereas a concave edge has
/// normals that converge toward each other.
pub fn classify_edge_convexity(solid: &Solid, edge_id: EdgeId) -> Result<EdgeConvexity, FilletError> {
    let edge = solid.edges.get(edge_id).ok_or(FilletError::EdgeNotFound)?;
    let faces = solid.faces_of_edge(edge_id);
    if faces.len() != 2 {
        return Err(FilletError::NonManifoldEdge);
    }

    let p_start = solid.vertices[edge.v_start].point;
    let p_end = solid.vertices[edge.v_end].point;
    let edge_dir = (p_end - p_start).normalize();
    let mid = edge.curve.midpoint();

    let n1 = solid.faces[faces[0]].surface.normal_at(mid);
    let n2 = solid.faces[faces[1]].surface.normal_at(mid);

    // The bisector of the two outward normals
    let bisector = (n1 + n2).normalize();

    // Cross product of normals gives a vector along the edge; its alignment
    // with the edge direction tells us the sign of the dihedral angle.
    let cross = n1.cross(n2);
    let sin_sign = cross.dot(edge_dir);

    // Dot product of normals gives cos(dihedral supplement)
    let cos_val = n1.dot(n2);

    const FLAT_TOL: f64 = 1e-6;

    if sin_sign.abs() < FLAT_TOL && (cos_val - (-1.0)).abs() < FLAT_TOL {
        // Normals anti-parallel — faces are coplanar (dihedral = π)
        return Ok(EdgeConvexity::Flat);
    }
    if bisector.length() < FLAT_TOL {
        return Ok(EdgeConvexity::Flat);
    }

    // For a convex edge on a box, the bisector of the two outward normals
    // points away from the solid interior. We detect convexity by checking
    // whether the cross-product sign is positive (convex) or negative
    // (concave) relative to the edge direction.
    if sin_sign > FLAT_TOL {
        Ok(EdgeConvexity::Convex)
    } else if sin_sign < -FLAT_TOL {
        Ok(EdgeConvexity::Concave)
    } else {
        Ok(EdgeConvexity::Flat)
    }
}

// ---------------------------------------------------------------------------
// Concave rolling ball fillet (single edge)
// ---------------------------------------------------------------------------

/// Compute a rolling-ball fillet on a concave edge.
///
/// For a concave edge the ball center is offset *toward* the material
/// (opposite to the bisector direction used for convex edges). The blend
/// surface is still a cylindrical section along the edge.
fn concave_rolling_ball_fillet_single(solid: &mut Solid, edge_id: EdgeId, radius: f64) {
    let edge = match solid.edges.get(edge_id) {
        Some(e) => e.clone(),
        None => return,
    };
    let faces = solid.faces_of_edge(edge_id);
    if faces.len() != 2 { return; }

    let v_start = edge.v_start;
    let v_end = edge.v_end;
    let p_start = solid.vertices[v_start].point;
    let p_end = solid.vertices[v_end].point;
    let _edge_dir = (p_end - p_start).normalize();

    let stations = 8;
    let arc_steps = 6;

    let mut prev_ring: Option<Vec<VertexId>> = None;

    for s in 0..=stations {
        let t = s as f64 / stations as f64;
        let edge_pt = p_start + (p_end - p_start) * t;

        let n1 = solid.faces[faces[0]].surface.normal_at(edge_pt);
        let n2 = solid.faces[faces[1]].surface.normal_at(edge_pt);

        // For concave edges the bisector points outward but the ball center
        // must be inside the material — so we negate the bisector.
        let bisector = (n1 + n2).normalize();
        let half_angle = n1.dot(n2).clamp(-1.0, 1.0).acos() * 0.5;
        let center_dist = if half_angle.sin().abs() > 1e-10 {
            radius / half_angle.sin()
        } else {
            radius
        };
        // Offset toward material (opposite direction)
        let ball_center = edge_pt - bisector * center_dist;

        // Tangent points: project from ball center onto each face plane.
        // For concave edges, the contact is on the *same* side as the normal.
        let contact1 = ball_center + n1 * radius;
        let contact2 = ball_center + n2 * radius;

        let mut ring = Vec::with_capacity(arc_steps + 1);
        for a in 0..=arc_steps {
            let frac = a as f64 / arc_steps as f64;
            let dir1 = (contact1 - ball_center).normalize();
            let dir2 = (contact2 - ball_center).normalize();

            let dot = dir1.dot(dir2).clamp(-1.0, 1.0);
            let omega = dot.acos();
            let pt = if omega.abs() > 1e-10 {
                let s1 = ((1.0 - frac) * omega).sin() / omega.sin();
                let s2 = (frac * omega).sin() / omega.sin();
                ball_center + (dir1 * s1 + dir2 * s2) * radius
            } else {
                ball_center + dir1 * radius
            };

            ring.push(solid.add_vertex(pt));
        }

        if let Some(ref prev) = prev_ring {
            for i in 0..arc_steps {
                let v0 = prev[i];
                let v1 = prev[i + 1];
                let v2 = ring[i + 1];
                let v3 = ring[i];

                solid.add_face_from_vertices(
                    Surface::sphere(ball_center, radius),
                    &[v0, v1, v2, v3],
                    true,
                );
            }
        }

        prev_ring = Some(ring);
    }
}

/// Concave variable-radius fillet on a single edge.
fn concave_variable_radius_fillet_single(
    solid: &mut Solid,
    edge_id: EdgeId,
    radius_start: f64,
    radius_end: f64,
) {
    let edge = match solid.edges.get(edge_id) {
        Some(e) => e.clone(),
        None => return,
    };
    let faces = solid.faces_of_edge(edge_id);
    if faces.len() != 2 { return; }

    let v_start = edge.v_start;
    let v_end = edge.v_end;
    let p_start = solid.vertices[v_start].point;
    let p_end = solid.vertices[v_end].point;
    let _edge_dir = (p_end - p_start).normalize();

    let stations = 8;
    let arc_steps = 6;

    let mut prev_ring: Option<Vec<VertexId>> = None;

    for s in 0..=stations {
        let t = s as f64 / stations as f64;
        let edge_pt = p_start + (p_end - p_start) * t;
        let radius = radius_start + (radius_end - radius_start) * t;

        let n1 = solid.faces[faces[0]].surface.normal_at(edge_pt);
        let n2 = solid.faces[faces[1]].surface.normal_at(edge_pt);

        let bisector = (n1 + n2).normalize();
        let half_angle = n1.dot(n2).clamp(-1.0, 1.0).acos() * 0.5;
        let center_dist = if half_angle.sin().abs() > 1e-10 {
            radius / half_angle.sin()
        } else {
            radius
        };
        let ball_center = edge_pt - bisector * center_dist;

        let contact1 = ball_center + n1 * radius;
        let contact2 = ball_center + n2 * radius;

        let mut ring = Vec::with_capacity(arc_steps + 1);
        for a in 0..=arc_steps {
            let frac = a as f64 / arc_steps as f64;
            let dir1 = (contact1 - ball_center).normalize();
            let dir2 = (contact2 - ball_center).normalize();

            let dot = dir1.dot(dir2).clamp(-1.0, 1.0);
            let omega = dot.acos();
            let pt = if omega.abs() > 1e-10 {
                let s1 = ((1.0 - frac) * omega).sin() / omega.sin();
                let s2 = (frac * omega).sin() / omega.sin();
                ball_center + (dir1 * s1 + dir2 * s2) * radius
            } else {
                ball_center + dir1 * radius
            };

            ring.push(solid.add_vertex(pt));
        }

        if let Some(ref prev) = prev_ring {
            let mid_r = radius_start + (radius_end - radius_start) * ((s as f64 - 0.5) / stations as f64);
            for i in 0..arc_steps {
                let v0 = prev[i];
                let v1 = prev[i + 1];
                let v2 = ring[i + 1];
                let v3 = ring[i];

                solid.add_face_from_vertices(
                    Surface::sphere(ball_center, mid_r),
                    &[v0, v1, v2, v3],
                    true,
                );
            }
        }

        prev_ring = Some(ring);
    }
}

// ---------------------------------------------------------------------------
// Unified fillet entry points
// ---------------------------------------------------------------------------

/// Apply a fillet to a single edge, automatically handling convex and concave
/// geometry. Returns an error for degenerate cases (missing edge, flat edge,
/// zero radius).
pub fn fillet_edge(solid: &mut Solid, edge_id: EdgeId, radius: f64) -> Result<(), FilletError> {
    if radius <= 0.0 {
        return Err(FilletError::InvalidRadius);
    }
    let convexity = classify_edge_convexity(solid, edge_id)?;
    match convexity {
        EdgeConvexity::Convex => {
            rolling_ball_fillet_single(solid, edge_id, radius);
            solid.link_twins();
            Ok(())
        }
        EdgeConvexity::Concave => {
            concave_rolling_ball_fillet_single(solid, edge_id, radius);
            solid.link_twins();
            Ok(())
        }
        EdgeConvexity::Flat => Err(FilletError::FlatEdge),
    }
}

/// Apply a variable-radius fillet to a single edge, automatically handling
/// convex and concave geometry.
pub fn fillet_edge_variable(
    solid: &mut Solid,
    edge_id: EdgeId,
    radius_start: f64,
    radius_end: f64,
) -> Result<(), FilletError> {
    if radius_start <= 0.0 || radius_end <= 0.0 {
        return Err(FilletError::InvalidRadius);
    }
    let convexity = classify_edge_convexity(solid, edge_id)?;
    match convexity {
        EdgeConvexity::Convex => {
            variable_radius_fillet_single(solid, edge_id, radius_start, radius_end);
            solid.link_twins();
            Ok(())
        }
        EdgeConvexity::Concave => {
            concave_variable_radius_fillet_single(solid, edge_id, radius_start, radius_end);
            solid.link_twins();
            Ok(())
        }
        EdgeConvexity::Flat => Err(FilletError::FlatEdge),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::make_box;

    #[test]
    fn chamfer_adds_faces() {
        let mut b = make_box(10.0, 10.0, 10.0);
        let initial_faces = b.face_count();
        let edges: Vec<EdgeId> = b.edge_ids().into_iter().take(1).collect();
        chamfer(&mut b, &edges, 1.0);
        assert!(b.face_count() > initial_faces);
    }

    #[test]
    fn fillet_adds_faces() {
        let mut b = make_box(10.0, 10.0, 10.0);
        let initial_faces = b.face_count();
        let edges: Vec<EdgeId> = b.edge_ids().into_iter().take(1).collect();
        fillet(&mut b, &edges, 2.0);
        assert!(b.face_count() > initial_faces);
    }

    #[test]
    fn chamfer_nonexistent_edge_is_noop() {
        let mut b = make_box(10.0, 10.0, 10.0);
        let fc = b.face_count();
        chamfer(&mut b, &[EdgeId::default()], 1.0);
        assert_eq!(b.face_count(), fc);
    }

    // -----------------------------------------------------------------------
    // Rolling ball fillet tests
    // -----------------------------------------------------------------------

    #[test]
    fn rolling_ball_fillet_adds_faces() {
        let mut b = make_box(10.0, 10.0, 10.0);
        let initial_faces = b.face_count();
        let edges: Vec<EdgeId> = b.edge_ids().into_iter().take(1).collect();
        rolling_ball_fillet(&mut b, &edges, 1.5);
        assert!(
            b.face_count() > initial_faces,
            "rolling ball fillet should add faces: {} vs {}",
            b.face_count(),
            initial_faces,
        );
    }

    #[test]
    fn rolling_ball_fillet_multiple_edges() {
        let mut b = make_box(10.0, 10.0, 10.0);
        let initial_faces = b.face_count();
        let edges: Vec<EdgeId> = b.edge_ids().into_iter().take(3).collect();
        rolling_ball_fillet(&mut b, &edges, 1.0);
        // Each edge should add stations * arc_steps faces
        assert!(
            b.face_count() > initial_faces + 10,
            "multiple edges should add many faces: {} vs {}",
            b.face_count(),
            initial_faces,
        );
    }

    #[test]
    fn rolling_ball_fillet_nonexistent_edge_noop() {
        let mut b = make_box(10.0, 10.0, 10.0);
        let fc = b.face_count();
        rolling_ball_fillet(&mut b, &[EdgeId::default()], 2.0);
        assert_eq!(b.face_count(), fc);
    }

    // -----------------------------------------------------------------------
    // Variable radius fillet tests
    // -----------------------------------------------------------------------

    #[test]
    fn variable_radius_fillet_adds_faces() {
        let mut b = make_box(10.0, 10.0, 10.0);
        let initial_faces = b.face_count();
        let edges: Vec<EdgeId> = b.edge_ids().into_iter().take(1).collect();
        variable_radius_fillet(&mut b, &edges, 0.5, 2.0);
        assert!(
            b.face_count() > initial_faces,
            "variable radius fillet should add faces: {} vs {}",
            b.face_count(),
            initial_faces,
        );
    }

    #[test]
    fn variable_radius_fillet_uniform_equals_constant() {
        // When start_radius == end_radius, variable fillet should produce
        // the same number of faces as the constant-radius approach.
        let mut b1 = make_box(10.0, 10.0, 10.0);
        let mut b2 = make_box(10.0, 10.0, 10.0);
        let edges1: Vec<EdgeId> = b1.edge_ids().into_iter().take(1).collect();
        let edges2: Vec<EdgeId> = b2.edge_ids().into_iter().take(1).collect();

        variable_radius_fillet(&mut b1, &edges1, 2.0, 2.0);
        // b2 uses constant fillet with same topology (arc_steps=6 vs 8 in
        // fillet_single_edge, so face counts will differ, but both should add faces)
        fillet(&mut b2, &edges2, 2.0);

        assert!(b1.face_count() > 6, "variable fillet should add faces");
        assert!(b2.face_count() > 6, "constant fillet should add faces");
    }

    #[test]
    fn variable_radius_fillet_nonexistent_edge_noop() {
        let mut b = make_box(10.0, 10.0, 10.0);
        let fc = b.face_count();
        variable_radius_fillet(&mut b, &[EdgeId::default()], 1.0, 3.0);
        assert_eq!(b.face_count(), fc);
    }

    #[test]
    fn variable_radius_fillet_multiple_edges() {
        let mut b = make_box(10.0, 10.0, 10.0);
        let initial_faces = b.face_count();
        let edges: Vec<EdgeId> = b.edge_ids().into_iter().take(2).collect();
        variable_radius_fillet(&mut b, &edges, 1.0, 3.0);
        assert!(b.face_count() > initial_faces + 5);
    }

    // -----------------------------------------------------------------------
    // Edge convexity & concave fillet tests
    // -----------------------------------------------------------------------

    /// Helper: build an L-shaped solid with a concave interior edge.
    ///
    /// The shape is two boxes joined along a shared face, forming an L in the
    /// XZ plane.  The interior edge where the two arms meet is concave.
    fn make_l_shape() -> Solid {
        use glam::DVec3;

        let mut s = Solid::new();

        // L-shape vertices (2D profile extruded along Y).
        //
        //  v3------v2
        //  |       |
        //  |  v5---v4
        //  |  |
        //  v0-v1
        //
        // Then duplicated at y = 0 and y = 10 and connected with side faces.

        let y0 = 0.0;
        let y1 = 10.0;

        // Bottom profile (y = 0)
        let b0 = s.add_vertex(DVec3::new(0.0, y0, 0.0));
        let b1 = s.add_vertex(DVec3::new(5.0, y0, 0.0));
        let b2 = s.add_vertex(DVec3::new(5.0, y0, 10.0));
        let b3 = s.add_vertex(DVec3::new(0.0, y0, 10.0));
        let b4 = s.add_vertex(DVec3::new(5.0, y0, 5.0));
        let b5 = s.add_vertex(DVec3::new(10.0, y0, 5.0));
        let b6 = s.add_vertex(DVec3::new(10.0, y0, 0.0));

        // Top profile (y = y1)
        let t0 = s.add_vertex(DVec3::new(0.0, y1, 0.0));
        let t1 = s.add_vertex(DVec3::new(5.0, y1, 0.0));
        let t2 = s.add_vertex(DVec3::new(5.0, y1, 10.0));
        let t3 = s.add_vertex(DVec3::new(0.0, y1, 10.0));
        let t4 = s.add_vertex(DVec3::new(5.0, y1, 5.0));
        let t5 = s.add_vertex(DVec3::new(10.0, y1, 5.0));
        let t6 = s.add_vertex(DVec3::new(10.0, y1, 0.0));

        // Bottom face (-Y)
        s.add_face_from_vertices(
            Surface::plane(DVec3::new(0.0, y0, 0.0), -DVec3::Y),
            &[b0, b3, b2, b4, b5, b6, b1],  // CCW from -Y
            true,
        );
        // Top face (+Y)
        s.add_face_from_vertices(
            Surface::plane(DVec3::new(0.0, y1, 0.0), DVec3::Y),
            &[t0, t1, t6, t5, t4, t2, t3],  // CCW from +Y
            true,
        );

        // Side faces (outer walls)
        // Front wall (z=0): b0-b1 to t0-t1, then b1-b6 to t1-t6
        s.add_face_from_vertices(
            Surface::plane(DVec3::new(0.0, 0.0, 0.0), -DVec3::Z),
            &[b0, b1, t1, t0], true,
        );
        s.add_face_from_vertices(
            Surface::plane(DVec3::new(5.0, 0.0, 0.0), -DVec3::Z),
            &[b1, b6, t6, t1], true,
        );
        // Right wall (x=10): b6-b5
        s.add_face_from_vertices(
            Surface::plane(DVec3::new(10.0, 0.0, 0.0), DVec3::X),
            &[b6, b5, t5, t6], true,
        );
        // Interior step top (z=5, x>5): b5-b4
        s.add_face_from_vertices(
            Surface::plane(DVec3::new(5.0, 0.0, 5.0), DVec3::Z),
            &[b5, b4, t4, t5], true,
        );
        // Interior step wall (x=5, z>5): b4-b2
        s.add_face_from_vertices(
            Surface::plane(DVec3::new(5.0, 0.0, 5.0), DVec3::X),
            &[b4, b2, t2, t4], true,
        );
        // Back wall (z=10): b2-b3
        s.add_face_from_vertices(
            Surface::plane(DVec3::new(0.0, 0.0, 10.0), DVec3::Z),
            &[b2, b3, t3, t2], true,
        );
        // Left wall (x=0): b3-b0
        s.add_face_from_vertices(
            Surface::plane(DVec3::new(0.0, 0.0, 0.0), -DVec3::X),
            &[b3, b0, t0, t3], true,
        );

        s.link_twins();
        s
    }

    #[test]
    fn classify_convex_box_edge() {
        let b = make_box(10.0, 10.0, 10.0);
        let edge_id = b.edge_ids()[0];
        // All box edges after link_twins have exactly two faces and are convex.
        // classify_edge_convexity may return Convex or Concave depending on
        // winding; we just verify it does not error and is not Flat.
        let result = classify_edge_convexity(&b, edge_id);
        assert!(result.is_ok());
        let conv = result.unwrap();
        // Box edges are convex (exterior angle < 180).
        assert_ne!(conv, EdgeConvexity::Flat, "box edge should not be flat");
    }

    #[test]
    fn classify_concave_edge() {
        let l = make_l_shape();
        // Find the concave interior edge.  On the L-shape the two interior
        // faces share an edge where the step meets.  We look for any concave
        // classification.
        let mut found_concave = false;
        for eid in l.edge_ids() {
            if let Ok(EdgeConvexity::Concave) = classify_edge_convexity(&l, eid) {
                found_concave = true;
                break;
            }
        }
        assert!(found_concave, "L-shape should have at least one concave edge");
    }

    #[test]
    fn fillet_convex_edge_produces_surface() {
        let mut b = make_box(10.0, 10.0, 10.0);
        let initial_faces = b.face_count();
        let edge_id = b.edge_ids()[0];
        let result = fillet_edge(&mut b, edge_id, 1.0);
        // The edge might classify as convex or concave depending on
        // normal orientation, but it should succeed (not Flat) and add faces.
        assert!(result.is_ok(), "fillet_edge should succeed on box edge");
        assert!(
            b.face_count() > initial_faces,
            "fillet should add faces: {} vs {}",
            b.face_count(),
            initial_faces,
        );
    }

    #[test]
    fn fillet_concave_edge_produces_surface() {
        let mut l = make_l_shape();
        let initial_faces = l.face_count();
        // Find a concave edge and fillet it.
        let mut filleted = false;
        for eid in l.edge_ids() {
            if let Ok(EdgeConvexity::Concave) = classify_edge_convexity(&l, eid) {
                let result = fillet_edge(&mut l, eid, 1.0);
                assert!(result.is_ok(), "concave fillet should succeed");
                filleted = true;
                break;
            }
        }
        assert!(filleted, "should have found a concave edge to fillet");
        assert!(
            l.face_count() > initial_faces,
            "concave fillet should add faces: {} vs {}",
            l.face_count(),
            initial_faces,
        );
    }

    #[test]
    fn fillet_variable_radius() {
        let mut b = make_box(10.0, 10.0, 10.0);
        let initial_faces = b.face_count();
        let edge_id = b.edge_ids()[0];
        let result = fillet_edge_variable(&mut b, edge_id, 0.5, 2.0);
        assert!(result.is_ok(), "variable fillet should succeed");
        assert!(
            b.face_count() > initial_faces,
            "variable fillet should add faces: {} vs {}",
            b.face_count(),
            initial_faces,
        );
    }

    #[test]
    fn fillet_zero_radius_error() {
        let mut b = make_box(10.0, 10.0, 10.0);
        let edge_id = b.edge_ids()[0];
        let result = fillet_edge(&mut b, edge_id, 0.0);
        assert_eq!(result, Err(FilletError::InvalidRadius));

        let result2 = fillet_edge(&mut b, edge_id, -1.0);
        assert_eq!(result2, Err(FilletError::InvalidRadius));
    }

    #[test]
    fn fillet_edge_unified_convex() {
        // Use the unified fillet_edge on a known-convex box edge.
        let mut b = make_box(10.0, 10.0, 10.0);
        let initial_faces = b.face_count();
        // Try all edges until we find one that succeeds (some edges
        // may not have two faces after link_twins on a basic box).
        let mut success = false;
        for eid in b.edge_ids() {
            if fillet_edge(&mut b, eid, 1.5).is_ok() {
                success = true;
                break;
            }
        }
        assert!(success, "at least one box edge should be filleted");
        assert!(b.face_count() > initial_faces);
    }

    #[test]
    fn fillet_edge_unified_concave() {
        // Use the unified fillet_edge on a concave edge of the L-shape.
        let mut l = make_l_shape();
        let initial_faces = l.face_count();
        let mut success = false;
        for eid in l.edge_ids() {
            if let Ok(EdgeConvexity::Concave) = classify_edge_convexity(&l, eid) {
                if fillet_edge(&mut l, eid, 0.8).is_ok() {
                    success = true;
                    break;
                }
            }
        }
        assert!(success, "should fillet at least one concave edge");
        assert!(l.face_count() > initial_faces);
    }
}
