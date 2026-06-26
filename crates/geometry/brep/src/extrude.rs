//! Extrude operation — create a solid from a 2D profile.

use glam::DVec3;
use crate::profile::{Profile, ProfileSegment};
use crate::solid::Solid;
use crate::surface::Surface;

/// Extrude a 2D profile along a direction to create a solid.
///
/// The profile is placed on the plane defined by `origin`, with axes `u`, `v`.
/// The extrusion goes along `direction` for the given `distance`.
pub fn extrude(
    profile: &Profile,
    origin: DVec3,
    u: DVec3,
    v: DVec3,
    direction: DVec3,
    distance: f64,
) -> Solid {
    let dir = direction.normalize() * distance;
    let mut solid = Solid::new();

    // For polygonal profiles: get 2D vertices, lift to 3D
    let pts_2d = profile.vertices_2d();
    let n = pts_2d.len();
    if n < 3 { return solid; }

    // Create bottom and top vertices
    let mut bottom_verts = Vec::with_capacity(n);
    let mut top_verts = Vec::with_capacity(n);

    for p in &pts_2d {
        let p3 = origin + u * p.x + v * p.y;
        bottom_verts.push(solid.add_vertex(p3));
        top_verts.push(solid.add_vertex(p3 + dir));
    }

    // Bottom face (reversed winding for outward normal pointing opposite to direction)
    let bottom_normal = -direction.normalize();
    let bottom_reversed: Vec<_> = bottom_verts.iter().rev().copied().collect();
    solid.add_face_from_vertices(
        Surface::plane(origin, bottom_normal),
        &bottom_reversed, true,
    );

    // Top face
    let top_origin = origin + dir;
    let top_normal = direction.normalize();
    solid.add_face_from_vertices(
        Surface::plane(top_origin, top_normal),
        &top_verts, true,
    );

    // Side faces — one quad per profile segment
    for i in 0..n {
        let j = (i + 1) % n;

        // Determine if this side is planar or cylindrical
        let seg = &profile.segments[i];
        let surface = match seg {
            ProfileSegment::Line { .. } => {
                // Compute face normal from the two bottom vertices and the extrude direction
                let p0 = solid.vertices[bottom_verts[i]].point;
                let p1 = solid.vertices[bottom_verts[j]].point;
                let edge_dir = (p1 - p0).normalize();
                let face_normal = edge_dir.cross(direction.normalize()).normalize();
                Surface::plane(p0, face_normal)
            }
            ProfileSegment::Arc { center, radius, .. } => {
                let center_3d = origin + u * center.x + v * center.y;
                Surface::cylinder(center_3d, direction.normalize(), *radius)
            }
        };

        solid.add_face_from_vertices(
            surface,
            &[bottom_verts[i], bottom_verts[j], top_verts[j], top_verts[i]], true,
        );
    }

    solid.link_twins();
    solid
}

/// Extrude a profile on the XY plane along Z.
pub fn extrude_z(profile: &Profile, distance: f64) -> Solid {
    extrude(profile, DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z, distance)
}

/// Extrude a profile symmetrically (equal distance in both directions).
pub fn extrude_symmetric(
    profile: &Profile,
    origin: DVec3,
    u: DVec3,
    v: DVec3,
    direction: DVec3,
    total_distance: f64,
) -> Solid {
    let half = total_distance / 2.0;
    let new_origin = origin - direction.normalize() * half;
    extrude(profile, new_origin, u, v, direction, total_distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;

    #[test]
    fn extrude_rectangle() {
        let profile = Profile::rectangle(10.0, 5.0);
        let solid = extrude_z(&profile, 20.0);
        // Rectangle has 4 vertices × 2 = 8
        assert_eq!(solid.vertex_count(), 8);
        // 2 caps + 4 sides = 6 faces
        assert_eq!(solid.face_count(), 6);
        // Box: 12 edges
        assert_eq!(solid.edge_count(), 12);
        assert!(solid.is_valid_shell());
    }

    #[test]
    fn extrude_l_shape() {
        let profile = Profile::l_shape(20.0, 30.0, 5.0);
        let solid = extrude_z(&profile, 10.0);
        // L-shape has 6 vertices × 2 = 12
        assert_eq!(solid.vertex_count(), 12);
        // 2 caps + 6 sides = 8 faces
        assert_eq!(solid.face_count(), 8);
        assert!(solid.is_valid_shell());
    }

    #[test]
    fn extrude_bounding_box() {
        let profile = Profile::rectangle(10.0, 5.0);
        let solid = extrude_z(&profile, 20.0);
        let (min, max) = solid.bounding_box();
        assert!((min.x + 5.0).abs() < 1e-10);
        assert!((max.x - 5.0).abs() < 1e-10);
        assert!((min.z - 0.0).abs() < 1e-10);
        assert!((max.z - 20.0).abs() < 1e-10);
    }

    #[test]
    fn extrude_symmetric_centered() {
        let profile = Profile::rectangle(10.0, 10.0);
        let solid = extrude_symmetric(
            &profile, DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z, 20.0
        );
        let (min, max) = solid.bounding_box();
        assert!((min.z + 10.0).abs() < 1e-10);
        assert!((max.z - 10.0).abs() < 1e-10);
    }
}
