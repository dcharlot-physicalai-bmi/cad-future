//! Pattern operations for the B-Rep kernel — linear pattern, circular pattern, and mirror.
//!
//! Each operation transforms (translates, rotates, or reflects) every vertex of a solid,
//! then recreates all faces with the transformed vertices and surfaces.

use std::f64::consts::TAU;

use glam::DVec3;

use crate::solid::Solid;
use crate::surface::Surface;

/// Rotate a point around an axis through `center` by `angle` radians (Rodrigues' formula).
fn rotate_point(point: DVec3, center: DVec3, axis: DVec3, angle: f64) -> DVec3 {
    let k = axis.normalize();
    let v = point - center;
    let rotated = v * angle.cos() + k.cross(v) * angle.sin() + k * k.dot(v) * (1.0 - angle.cos());
    center + rotated
}

/// Reflect a point across a plane defined by a point and a normal.
fn reflect_point(point: DVec3, plane_point: DVec3, plane_normal: DVec3) -> DVec3 {
    let n = plane_normal.normalize();
    point - 2.0 * (point - plane_point).dot(n) * n
}

/// Translate a surface by an offset vector.
fn translate_surface(surface: &Surface, offset: DVec3) -> Surface {
    match surface {
        Surface::Plane { origin, normal } => Surface::Plane {
            origin: *origin + offset,
            normal: *normal,
        },
        Surface::Cylinder { origin, axis, radius } => Surface::Cylinder {
            origin: *origin + offset,
            axis: *axis,
            radius: *radius,
        },
        Surface::Sphere { center, radius } => Surface::Sphere {
            center: *center + offset,
            radius: *radius,
        },
        Surface::Torus { center, axis, major_radius, minor_radius } => Surface::Torus {
            center: *center + offset,
            axis: *axis,
            major_radius: *major_radius,
            minor_radius: *minor_radius,
        },
        Surface::Cone { apex, axis, half_angle } => Surface::Cone {
            apex: *apex + offset,
            axis: *axis,
            half_angle: *half_angle,
        },
        Surface::Nurbs { control_points, weights, knots_u, knots_v, degree_u, degree_v } => {
            let translated_cp = control_points
                .iter()
                .map(|row| row.iter().map(|p| *p + offset).collect())
                .collect();
            Surface::Nurbs {
                control_points: translated_cp,
                weights: weights.clone(),
                knots_u: knots_u.clone(),
                knots_v: knots_v.clone(),
                degree_u: *degree_u,
                degree_v: *degree_v,
            }
        }
    }
}

/// Rotate a surface around an axis through `center` by `angle` radians.
fn rotate_surface(surface: &Surface, center: DVec3, axis: DVec3, angle: f64) -> Surface {
    match surface {
        Surface::Plane { origin, normal } => Surface::Plane {
            origin: rotate_point(*origin, center, axis, angle),
            normal: rotate_point(*normal, DVec3::ZERO, axis, angle),
        },
        Surface::Cylinder { origin, axis: cyl_axis, radius } => Surface::Cylinder {
            origin: rotate_point(*origin, center, axis, angle),
            axis: rotate_point(*cyl_axis, DVec3::ZERO, axis, angle),
            radius: *radius,
        },
        Surface::Sphere { center: sph_center, radius } => Surface::Sphere {
            center: rotate_point(*sph_center, center, axis, angle),
            radius: *radius,
        },
        Surface::Torus { center: tor_center, axis: tor_axis, major_radius, minor_radius } => {
            Surface::Torus {
                center: rotate_point(*tor_center, center, axis, angle),
                axis: rotate_point(*tor_axis, DVec3::ZERO, axis, angle),
                major_radius: *major_radius,
                minor_radius: *minor_radius,
            }
        }
        Surface::Cone { apex, axis: cone_axis, half_angle } => Surface::Cone {
            apex: rotate_point(*apex, center, axis, angle),
            axis: rotate_point(*cone_axis, DVec3::ZERO, axis, angle),
            half_angle: *half_angle,
        },
        Surface::Nurbs { control_points, weights, knots_u, knots_v, degree_u, degree_v } => {
            let rotated_cp = control_points
                .iter()
                .map(|row| row.iter().map(|p| rotate_point(*p, center, axis, angle)).collect())
                .collect();
            Surface::Nurbs {
                control_points: rotated_cp,
                weights: weights.clone(),
                knots_u: knots_u.clone(),
                knots_v: knots_v.clone(),
                degree_u: *degree_u,
                degree_v: *degree_v,
            }
        }
    }
}

/// Reflect a surface across a plane defined by a point and normal.
/// The normal is also flipped since reflection reverses orientation.
fn reflect_surface(surface: &Surface, plane_point: DVec3, plane_normal: DVec3) -> Surface {
    let n = plane_normal.normalize();
    let reflect_dir = |d: DVec3| d - 2.0 * d.dot(n) * n;

    match surface {
        Surface::Plane { origin, normal } => Surface::Plane {
            origin: reflect_point(*origin, plane_point, plane_normal),
            normal: reflect_dir(*normal),
        },
        Surface::Cylinder { origin, axis, radius } => Surface::Cylinder {
            origin: reflect_point(*origin, plane_point, plane_normal),
            axis: reflect_dir(*axis),
            radius: *radius,
        },
        Surface::Sphere { center, radius } => Surface::Sphere {
            center: reflect_point(*center, plane_point, plane_normal),
            radius: *radius,
        },
        Surface::Torus { center, axis, major_radius, minor_radius } => Surface::Torus {
            center: reflect_point(*center, plane_point, plane_normal),
            axis: reflect_dir(*axis),
            major_radius: *major_radius,
            minor_radius: *minor_radius,
        },
        Surface::Cone { apex, axis, half_angle } => Surface::Cone {
            apex: reflect_point(*apex, plane_point, plane_normal),
            axis: reflect_dir(*axis),
            half_angle: *half_angle,
        },
        Surface::Nurbs { control_points, weights, knots_u, knots_v, degree_u, degree_v } => {
            let reflected_cp = control_points
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|p| reflect_point(*p, plane_point, plane_normal))
                        .collect()
                })
                .collect();
            Surface::Nurbs {
                control_points: reflected_cp,
                weights: weights.clone(),
                knots_u: knots_u.clone(),
                knots_v: knots_v.clone(),
                degree_u: *degree_u,
                degree_v: *degree_v,
            }
        }
    }
}

/// Create a linear pattern (array) of a solid.
///
/// Returns a new solid containing `count` copies spaced by `spacing` along `direction`.
/// Copy 0 is at the original position; copy `count-1` is at offset `direction * spacing * (count-1)`.
pub fn linear_pattern(solid: &Solid, direction: DVec3, spacing: f64, count: usize) -> Solid {
    let mut result = Solid::new();
    if count == 0 {
        return result;
    }

    let dir = direction.normalize();
    let face_ids = solid.face_ids();

    // Collect face data once: (surface, ordered vertex points)
    let face_data: Vec<(Surface, Vec<DVec3>)> = face_ids
        .iter()
        .map(|&fid| {
            let surface = solid.faces[fid].surface.clone();
            let verts = solid.face_vertices(fid);
            let outward = solid.faces[fid].normal_outward;
            // We store outward info by keeping the surface as-is; outward flag is preserved below.
            let _ = outward;
            (surface, verts)
        })
        .collect();

    for i in 0..count {
        let offset = dir * spacing * i as f64;

        // Add all vertices for this copy and build a position -> id map
        let mut vert_ids = Vec::new();
        for (_, v) in &solid.vertices {
            let new_point = v.point + offset;
            let vid = result.add_vertex(new_point);
            vert_ids.push(vid);
        }

        // Recreate each face
        for (idx, &fid) in face_ids.iter().enumerate() {
            let (ref surface, ref verts) = face_data[idx];
            let outward = solid.faces[fid].normal_outward;
            let translated_surface = translate_surface(surface, offset);

            // Map face vertex points to new vertex IDs
            let new_face_verts: Vec<_> = verts
                .iter()
                .map(|pt| {
                    let target = *pt + offset;
                    // Find the matching new vertex
                    vert_ids
                        .iter()
                        .copied()
                        .find(|&vid| (result.vertices[vid].point - target).length() < 1e-12)
                        .expect("vertex not found in pattern copy")
                })
                .collect();

            result.add_face_from_vertices(translated_surface, &new_face_verts, outward);
        }
    }

    result.link_twins();
    result
}

/// Create a circular pattern of a solid.
///
/// `count` copies are evenly distributed around `axis` passing through `center`.
/// Copy 0 is at the original position; subsequent copies are rotated by `TAU * i / count`.
pub fn circular_pattern(solid: &Solid, center: DVec3, axis: DVec3, count: usize) -> Solid {
    let mut result = Solid::new();
    if count == 0 {
        return result;
    }

    let face_ids = solid.face_ids();

    let face_data: Vec<(Surface, Vec<DVec3>, bool)> = face_ids
        .iter()
        .map(|&fid| {
            let surface = solid.faces[fid].surface.clone();
            let verts = solid.face_vertices(fid);
            let outward = solid.faces[fid].normal_outward;
            (surface, verts, outward)
        })
        .collect();

    for i in 0..count {
        let angle = TAU * i as f64 / count as f64;

        // Add rotated vertices
        let mut vert_ids = Vec::new();
        for (_, v) in &solid.vertices {
            let new_point = rotate_point(v.point, center, axis, angle);
            let vid = result.add_vertex(new_point);
            vert_ids.push(vid);
        }

        // Recreate faces
        for (idx, _fid) in face_ids.iter().enumerate() {
            let (ref surface, ref verts, outward) = face_data[idx];
            let rotated_surface = rotate_surface(surface, center, axis, angle);

            let new_face_verts: Vec<_> = verts
                .iter()
                .map(|pt| {
                    let target = rotate_point(*pt, center, axis, angle);
                    vert_ids
                        .iter()
                        .copied()
                        .find(|&vid| (result.vertices[vid].point - target).length() < 1e-12)
                        .expect("vertex not found in circular pattern copy")
                })
                .collect();

            result.add_face_from_vertices(rotated_surface, &new_face_verts, outward);
        }
    }

    result.link_twins();
    result
}

/// Mirror a solid across a plane defined by a point and normal.
///
/// Reflection flips the orientation of faces, so the winding order is reversed
/// and surface normals are flipped accordingly.
pub fn mirror(solid: &Solid, plane_point: DVec3, plane_normal: DVec3) -> Solid {
    let mut result = Solid::new();

    let face_ids = solid.face_ids();

    // Add all reflected vertices
    let mut vert_ids = Vec::new();
    for (_, v) in &solid.vertices {
        let reflected = reflect_point(v.point, plane_point, plane_normal);
        let vid = result.add_vertex(reflected);
        vert_ids.push(vid);
    }

    // Recreate faces with reversed winding
    for &fid in &face_ids {
        let surface = &solid.faces[fid].surface;
        let verts = solid.face_vertices(fid);
        let outward = solid.faces[fid].normal_outward;
        let reflected_surface = reflect_surface(surface, plane_point, plane_normal);

        // Map and reverse winding order (reflection flips orientation)
        let mut new_face_verts: Vec<_> = verts
            .iter()
            .map(|pt| {
                let target = reflect_point(*pt, plane_point, plane_normal);
                vert_ids
                    .iter()
                    .copied()
                    .find(|&vid| (result.vertices[vid].point - target).length() < 1e-12)
                    .expect("vertex not found in mirror")
            })
            .collect();

        new_face_verts.reverse();

        result.add_face_from_vertices(reflected_surface, &new_face_verts, outward);
    }

    result.link_twins();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::make_box;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10
    }

    #[test]
    fn linear_pattern_three_boxes() {
        let unit_box = make_box(1.0, 1.0, 1.0);
        let orig_verts = unit_box.vertex_count();
        let orig_faces = unit_box.face_count();

        let patterned = linear_pattern(&unit_box, DVec3::X, 2.0, 3);

        assert_eq!(patterned.vertex_count(), orig_verts * 3);
        assert_eq!(patterned.face_count(), orig_faces * 3);

        // Check bounding box: original is [-0.5, 0.5], copies at x+2, x+4
        let (min, max) = patterned.bounding_box();
        assert!(approx_eq(min.x, -0.5));
        assert!(approx_eq(max.x, 4.5)); // 4.0 + 0.5
        assert!(approx_eq(min.y, -0.5));
        assert!(approx_eq(max.y, 0.5));
    }

    #[test]
    fn circular_pattern_four() {
        // Place a box off-center, then pattern 4x around Y axis
        let unit_box = make_box(1.0, 1.0, 1.0);
        let orig_verts = unit_box.vertex_count();
        let orig_faces = unit_box.face_count();

        // Build a box centered at x=5 so it orbits around Y.
        let offset_box = {
            let mut s = Solid::new();
            let cx = 5.0;
            let hw = 0.5;
            let v0 = s.add_vertex(DVec3::new(cx - hw, -hw, -hw));
            let v1 = s.add_vertex(DVec3::new(cx + hw, -hw, -hw));
            let v2 = s.add_vertex(DVec3::new(cx + hw,  hw, -hw));
            let v3 = s.add_vertex(DVec3::new(cx - hw,  hw, -hw));
            let v4 = s.add_vertex(DVec3::new(cx - hw, -hw,  hw));
            let v5 = s.add_vertex(DVec3::new(cx + hw, -hw,  hw));
            let v6 = s.add_vertex(DVec3::new(cx + hw,  hw,  hw));
            let v7 = s.add_vertex(DVec3::new(cx - hw,  hw,  hw));
            s.add_face_from_vertices(Surface::plane(DVec3::new(cx, 0.0,  hw), DVec3::Z), &[v4, v5, v6, v7], true);
            s.add_face_from_vertices(Surface::plane(DVec3::new(cx, 0.0, -hw), -DVec3::Z), &[v1, v0, v3, v2], true);
            s.add_face_from_vertices(Surface::plane(DVec3::new(cx + hw, 0.0, 0.0), DVec3::X), &[v5, v1, v2, v6], true);
            s.add_face_from_vertices(Surface::plane(DVec3::new(cx - hw, 0.0, 0.0), -DVec3::X), &[v0, v4, v7, v3], true);
            s.add_face_from_vertices(Surface::plane(DVec3::new(cx,  hw, 0.0), DVec3::Y), &[v3, v7, v6, v2], true);
            s.add_face_from_vertices(Surface::plane(DVec3::new(cx, -hw, 0.0), -DVec3::Y), &[v0, v1, v5, v4], true);
            s.link_twins();
            s
        };

        let patterned = circular_pattern(&offset_box, DVec3::ZERO, DVec3::Y, 4);

        assert_eq!(patterned.vertex_count(), orig_verts * 4);
        assert_eq!(patterned.face_count(), orig_faces * 4);

        // The four copies should be at roughly x=5, z=5, x=-5, z=-5
        let (min, max) = patterned.bounding_box();
        assert!(max.x > 4.0);
        assert!(max.z > 4.0);
        assert!(min.x < -4.0);
        assert!(min.z < -4.0);
    }

    #[test]
    fn mirror_box_across_yz() {
        // Create a box centered at x=3, mirror across YZ plane (x=0)
        let unit_box = make_box(1.0, 1.0, 1.0);
        // Use linear_pattern with count=2 and spacing=3.0 to get a copy at x=3.
        // Then take just the second copy by mirroring the 2-copy result? No, simpler:
        // offset the box by translating via linear_pattern(count=1, spacing ignored) -> copy at origin,
        // so instead build an offset solid from the 2-copy pattern's bounding region.
        // Simplest: just mirror the box at origin and check symmetry.
        let offset_box = linear_pattern(&unit_box, DVec3::X, 3.0, 2);
        // This has copies at x=0 and x=3. Bounding box: [-0.5, 3.5]

        let mirrored = mirror(&offset_box, DVec3::ZERO, DVec3::X);

        // Mirrored should have bounding box: [-3.5, 0.5]
        let (min, max) = mirrored.bounding_box();
        assert!(approx_eq(min.x, -3.5));
        assert!(approx_eq(max.x, 0.5));
        assert!(approx_eq(min.y, -0.5));
        assert!(approx_eq(max.y, 0.5));
    }

    #[test]
    fn mirror_reverses_normals() {
        let unit_box = make_box(1.0, 1.0, 1.0);
        let mirrored = mirror(&unit_box, DVec3::ZERO, DVec3::X);

        // For each face, check that the surface normal at the face centroid
        // points away from the solid center (outward).
        for fid in mirrored.face_ids() {
            let verts = mirrored.face_vertices(fid);
            let centroid = verts.iter().copied().sum::<DVec3>() / verts.len() as f64;
            let surface = &mirrored.faces[fid].surface;
            let normal = surface.normal_at(centroid);

            // For a box centered at origin, the face centroid direction and the
            // outward normal should agree in sign (dot product > 0).
            let dot = centroid.dot(normal);
            assert!(
                dot > -1e-10,
                "Face normal should point outward: centroid={centroid:?}, normal={normal:?}, dot={dot}"
            );
        }
    }

    #[test]
    fn linear_pattern_single_is_copy() {
        let unit_box = make_box(2.0, 3.0, 4.0);
        let copy = linear_pattern(&unit_box, DVec3::X, 10.0, 1);

        assert_eq!(copy.vertex_count(), unit_box.vertex_count());
        assert_eq!(copy.face_count(), unit_box.face_count());
        assert_eq!(copy.edge_count(), unit_box.edge_count());

        // Same bounding box
        let (orig_min, orig_max) = unit_box.bounding_box();
        let (copy_min, copy_max) = copy.bounding_box();
        assert!(approx_eq(orig_min.x, copy_min.x));
        assert!(approx_eq(orig_max.x, copy_max.x));
        assert!(approx_eq(orig_min.y, copy_min.y));
        assert!(approx_eq(orig_max.y, copy_max.y));
        assert!(approx_eq(orig_min.z, copy_min.z));
        assert!(approx_eq(orig_max.z, copy_max.z));
    }
}
