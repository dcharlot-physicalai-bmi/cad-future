//! Loft operation — create a solid by blending between two or more 3D profile
//! cross-sections.
//!
//! The general `loft` function accepts an ordered slice of sections where each
//! section is a closed polygon represented as a `Vec<DVec3>`.  All sections must
//! contain the same number of vertices.  The operation connects corresponding
//! vertices of adjacent sections with quad side faces and closes the solid with
//! a bottom cap (first section) and a top cap (last section).
//!
//! The convenience helper `loft_profiles` takes two `Profile` values (2D closed
//! loops) and places them at different Z heights before delegating to `loft`.
//!
//! ## NURBS loft
//!
//! `loft_nurbs` takes N cross-section curves (as `Curve` values) and creates a
//! tensor-product NURBS surface that interpolates them. Curves with differing
//! knot counts are reparameterized to a common sampling resolution.
//!
//! `loft_nurbs_guided` adds optional guide curves that shape the surface between
//! cross-sections.

use glam::{DVec2, DVec3};

use crate::curve::Curve;
use crate::profile::Profile;
use crate::solid::Solid;
use crate::surface::{Surface, nurbs_surface_uniform};
use crate::types::VertexId;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the centroid of a set of 3D points.
fn centroid(pts: &[DVec3]) -> DVec3 {
    let sum: DVec3 = pts.iter().copied().sum();
    sum / pts.len() as f64
}

/// Compute the outward-facing normal of a planar polygon from its vertex loop
/// using the Newell method.
fn polygon_normal(pts: &[DVec3]) -> DVec3 {
    let n = pts.len();
    let mut normal = DVec3::ZERO;
    for i in 0..n {
        let cur = pts[i];
        let next = pts[(i + 1) % n];
        normal.x += (cur.y - next.y) * (cur.z + next.z);
        normal.y += (cur.z - next.z) * (cur.x + next.x);
        normal.z += (cur.x - next.x) * (cur.y + next.y);
    }
    normal.normalize()
}

/// Compute a plane `Surface` for a quad defined by four points.  The normal is
/// derived from the cross product of two edges.
fn quad_plane(p0: DVec3, p1: DVec3, p2: DVec3) -> Surface {
    let e1 = p1 - p0;
    let e2 = p2 - p1;
    let normal = e1.cross(e2).normalize();
    Surface::plane(p0, normal)
}

// ---------------------------------------------------------------------------
// Core loft
// ---------------------------------------------------------------------------

/// Loft between two or more cross-sections.
///
/// Each section is a list of ordered 3D points forming a closed polygon.  All
/// sections **must** contain the same number of vertices.  Returns a closed
/// `Solid` with:
///
/// * A bottom cap (first section, reversed winding for outward normal).
/// * A top cap (last section).
/// * Quad side faces connecting corresponding edges of adjacent sections.
///
/// # Panics
///
/// Panics if fewer than two sections are provided or if sections have differing
/// vertex counts.
pub fn loft(sections: &[Vec<DVec3>]) -> Solid {
    assert!(sections.len() >= 2, "loft requires at least 2 sections");
    let vert_count = sections[0].len();
    assert!(vert_count >= 3, "each section must have at least 3 vertices");
    for (i, sec) in sections.iter().enumerate() {
        assert_eq!(
            sec.len(),
            vert_count,
            "section {i} has {} vertices, expected {vert_count}",
            sec.len()
        );
    }

    let mut solid = Solid::new();

    // ---- Create all vertices (section_count x vert_count) ----
    let mut ids: Vec<Vec<VertexId>> = Vec::with_capacity(sections.len());
    for sec in sections {
        let row: Vec<VertexId> = sec.iter().map(|&p| solid.add_vertex(p)).collect();
        ids.push(row);
    }

    // ---- Bottom cap (first section, reversed winding) ----
    let bottom = &sections[0];
    let bottom_normal = -polygon_normal(bottom);
    let bottom_origin = centroid(bottom);
    let bottom_reversed: Vec<VertexId> = ids[0].iter().rev().copied().collect();
    solid.add_face_from_vertices(
        Surface::plane(bottom_origin, bottom_normal),
        &bottom_reversed,
        true,
    );

    // ---- Top cap (last section) ----
    let top = sections.last().unwrap();
    let top_normal = polygon_normal(top);
    let top_origin = centroid(top);
    solid.add_face_from_vertices(
        Surface::plane(top_origin, top_normal),
        ids.last().unwrap(),
        true,
    );

    // ---- Side faces (quads between adjacent sections) ----
    for s in 0..sections.len() - 1 {
        for i in 0..vert_count {
            let j = (i + 1) % vert_count;

            // Quad: bottom-left, bottom-right, top-right, top-left
            let bl = ids[s][i];
            let br = ids[s][j];
            let tr = ids[s + 1][j];
            let tl = ids[s + 1][i];

            let p0 = sections[s][i];
            let p1 = sections[s][j];
            let p2 = sections[s + 1][j];

            let surface = quad_plane(p0, p1, p2);
            solid.add_face_from_vertices(surface, &[bl, br, tr, tl], true);
        }
    }

    solid.link_twins();
    solid
}

// ---------------------------------------------------------------------------
// Profile convenience helper
// ---------------------------------------------------------------------------

/// Loft between two 2D profiles placed at different heights along the Z axis.
///
/// Both profiles are lifted onto the XY plane at their respective Z values.
/// They must produce the same number of vertices (i.e. same segment count for
/// polygonal profiles).
pub fn loft_profiles(
    bottom_profile: &Profile,
    top_profile: &Profile,
    bottom_z: f64,
    top_z: f64,
) -> Solid {
    let lift = |verts: &[DVec2], z: f64| -> Vec<DVec3> {
        verts.iter().map(|v| DVec3::new(v.x, v.y, z)).collect()
    };

    let bottom_2d = bottom_profile.vertices_2d();
    let top_2d = top_profile.vertices_2d();

    let bottom_3d = lift(&bottom_2d, bottom_z);
    let top_3d = lift(&top_2d, top_z);

    loft(&[bottom_3d, top_3d])
}

// ---------------------------------------------------------------------------
// NURBS loft: N cross-section curves -> NURBS surface
// ---------------------------------------------------------------------------

/// Sample a curve at `count` evenly-spaced parameter values in [0, 1].
fn sample_curve(curve: &Curve, count: usize) -> Vec<DVec3> {
    (0..count)
        .map(|i| {
            let t = i as f64 / (count - 1).max(1) as f64;
            curve.evaluate(t)
        })
        .collect()
}

/// Loft N cross-section curves into a NURBS surface.
///
/// Curves with different numbers of control points are reparameterized by
/// uniform sampling so all sections share the same resolution. The resulting
/// surface has the sections in the V direction and the profile shape in U.
///
/// Returns a `Surface::Nurbs` (not a Solid — callers can tessellate or use it
/// in a face).
///
/// # Arguments
/// * `profiles` — at least 2 curves to loft between.
/// * `samples_u` — number of sample points per profile (controls U resolution).
///
/// # Panics
/// Panics if fewer than 2 profiles are given.
pub fn loft_nurbs(profiles: &[Curve], samples_u: usize) -> Surface {
    assert!(profiles.len() >= 2, "loft_nurbs requires at least 2 profiles");
    let samples_u = samples_u.max(2);
    let n_sections = profiles.len();

    // Sample each profile at uniform parameter intervals
    let sections: Vec<Vec<DVec3>> = profiles
        .iter()
        .map(|c| sample_curve(c, samples_u))
        .collect();

    // Build control point grid: rows = sections (V), cols = samples_u (U)
    // We transpose so control_points[i][j] = row i (U index), col j... wait:
    // nurbs_surface_point iterates rows in U, cols in V, so we want:
    //   control_points[u_idx][v_idx]
    let mut control_points = vec![vec![DVec3::ZERO; n_sections]; samples_u];
    for u_idx in 0..samples_u {
        for v_idx in 0..n_sections {
            control_points[u_idx][v_idx] = sections[v_idx][u_idx];
        }
    }

    let degree_u = (samples_u - 1).min(3);
    let degree_v = (n_sections - 1).min(3);

    nurbs_surface_uniform(control_points, degree_u, degree_v)
}

/// Loft N cross-section curves with optional guide curves shaping the surface.
///
/// Guide curves run between sections (in the V direction) and pull the
/// interpolated surface toward their trajectories. Each guide is sampled at
/// `n_sections` points, and the influence is blended additively onto the
/// linearly-interpolated section points.
///
/// # Arguments
/// * `profiles` — at least 2 cross-section curves.
/// * `guides` — zero or more guide curves (evaluated over [0, 1] mapping to
///   first..last section).
/// * `samples_u` — resolution of each profile.
pub fn loft_nurbs_guided(
    profiles: &[Curve],
    guides: &[Curve],
    samples_u: usize,
) -> Surface {
    if guides.is_empty() {
        return loft_nurbs(profiles, samples_u);
    }

    assert!(profiles.len() >= 2, "loft_nurbs_guided requires at least 2 profiles");
    let samples_u = samples_u.max(2);

    // When guides are provided, we need enough sections in V to capture the
    // guide shape. Use at least 8 internal sections even if only 2 profiles
    // are given.
    let n_v = profiles.len().max(8);

    // Sample each input profile
    let input_sections: Vec<Vec<DVec3>> = profiles
        .iter()
        .map(|c| sample_curve(c, samples_u))
        .collect();

    // Build interpolated sections at n_v positions by linearly blending
    // between the nearest input profiles.
    let sections: Vec<Vec<DVec3>> = (0..n_v).map(|v_idx| {
        let t = v_idx as f64 / (n_v - 1).max(1) as f64;
        // Map t to input profile space
        let profile_t = t * (profiles.len() - 1) as f64;
        let lo = (profile_t as usize).min(profiles.len() - 2);
        let frac = profile_t - lo as f64;
        input_sections[lo].iter()
            .zip(input_sections[lo + 1].iter())
            .map(|(a, b)| *a * (1.0 - frac) + *b * frac)
            .collect()
    }).collect();

    // Sample each guide at n_v positions
    let guide_samples: Vec<Vec<DVec3>> = guides
        .iter()
        .map(|g| sample_curve(g, n_v))
        .collect();

    // Build the control point grid with guide influence.
    // For each V station, compute the guide's deviation from a straight line
    // between its endpoints, and apply that deviation to all U points.
    let mut control_points = vec![vec![DVec3::ZERO; n_v]; samples_u];

    for v_idx in 0..n_v {
        let section = &sections[v_idx];
        let t_v = v_idx as f64 / (n_v - 1).max(1) as f64;

        let mut avg_deviation = DVec3::ZERO;
        for g_pts in &guide_samples {
            let guide_pt = g_pts[v_idx];
            let g_start = g_pts[0];
            let g_end = g_pts[n_v - 1];
            let linear_guide = g_start * (1.0 - t_v) + g_end * t_v;
            avg_deviation += guide_pt - linear_guide;
        }
        if !guide_samples.is_empty() {
            avg_deviation /= guide_samples.len() as f64;
        }

        for u_idx in 0..samples_u {
            control_points[u_idx][v_idx] = section[u_idx] + avg_deviation;
        }
    }

    let degree_u = (samples_u - 1).min(3);
    let degree_v = (n_v - 1).min(3);

    nurbs_surface_uniform(control_points, degree_u, degree_v)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use crate::curve::{Curve, nurbs_uniform};

    /// Helper: build a rectangle section at a given Z with the given half-widths.
    fn rect_section(hw: f64, hh: f64, z: f64) -> Vec<DVec3> {
        vec![
            DVec3::new(-hw, -hh, z),
            DVec3::new(hw, -hh, z),
            DVec3::new(hw, hh, z),
            DVec3::new(-hw, hh, z),
        ]
    }

    #[test]
    fn loft_two_rectangles() {
        let bottom = rect_section(5.0, 3.0, 0.0);
        let top = rect_section(5.0, 3.0, 10.0);
        let solid = loft(&[bottom, top]);

        assert_eq!(solid.vertex_count(), 8);
        assert_eq!(solid.face_count(), 6);
        assert_eq!(solid.edge_count(), 12);
        assert!(solid.is_valid_shell());
    }

    #[test]
    fn loft_taper() {
        let bottom = rect_section(10.0, 10.0, 0.0);
        let top = rect_section(2.0, 2.0, 15.0);
        let solid = loft(&[bottom, top]);

        assert_eq!(solid.vertex_count(), 8);
        assert_eq!(solid.face_count(), 6);
        assert_eq!(solid.edge_count(), 12);
        assert!(solid.is_valid_shell());

        let (min, max) = solid.bounding_box();
        assert!((min.x + 10.0).abs() < 1e-10);
        assert!((max.x - 10.0).abs() < 1e-10);
        assert!((min.z - 0.0).abs() < 1e-10);
        assert!((max.z - 15.0).abs() < 1e-10);
    }

    #[test]
    fn loft_three_sections() {
        let s0 = rect_section(10.0, 10.0, 0.0);
        let s1 = rect_section(3.0, 3.0, 10.0);
        let s2 = rect_section(8.0, 8.0, 20.0);
        let solid = loft(&[s0, s1, s2]);

        assert_eq!(solid.vertex_count(), 12);
        assert_eq!(solid.face_count(), 10);
        assert_eq!(solid.edge_count(), 20);
        assert!(solid.is_valid_shell());
    }

    #[test]
    fn loft_valid_shell() {
        let bottom = rect_section(1.0, 1.0, 0.0);
        let top = rect_section(1.0, 1.0, 5.0);
        let solid = loft(&[bottom, top]);
        assert_eq!(solid.euler_characteristic(), 2);
    }

    #[test]
    fn loft_profiles_convenience() {
        let bottom_prof = Profile::rectangle(20.0, 10.0);
        let top_prof = Profile::rectangle(10.0, 5.0);
        let solid = loft_profiles(&bottom_prof, &top_prof, 0.0, 12.0);

        assert_eq!(solid.vertex_count(), 8);
        assert_eq!(solid.face_count(), 6);
        assert_eq!(solid.edge_count(), 12);
        assert!(solid.is_valid_shell());

        let (min, max) = solid.bounding_box();
        assert!((min.z - 0.0).abs() < 1e-10);
        assert!((max.z - 12.0).abs() < 1e-10);
    }

    #[test]
    #[should_panic(expected = "loft requires at least 2 sections")]
    fn loft_single_section_panics() {
        let section = rect_section(1.0, 1.0, 0.0);
        loft(&[section]);
    }

    #[test]
    #[should_panic(expected = "section 1 has 3 vertices, expected 4")]
    fn loft_mismatched_vertex_count_panics() {
        let s0 = rect_section(1.0, 1.0, 0.0);
        let s1 = vec![
            DVec3::new(0.0, 0.0, 5.0),
            DVec3::new(1.0, 0.0, 5.0),
            DVec3::new(0.5, 1.0, 5.0),
        ];
        loft(&[s0, s1]);
    }

    // -----------------------------------------------------------------------
    // NURBS loft tests
    // -----------------------------------------------------------------------

    /// Helper: create a line curve from a to b.
    fn line_curve(a: DVec3, b: DVec3) -> Curve {
        Curve::line(a, b)
    }

    #[test]
    fn loft_nurbs_two_lines() {
        // Loft between two parallel line segments at different heights
        let c1 = line_curve(DVec3::new(-5.0, 0.0, 0.0), DVec3::new(5.0, 0.0, 0.0));
        let c2 = line_curve(DVec3::new(-5.0, 0.0, 10.0), DVec3::new(5.0, 0.0, 10.0));
        let surf = loft_nurbs(&[c1, c2], 8);

        // Surface at center should be midway in Z
        let mid = surf.point_at(0.5, 0.5);
        assert!((mid.z - 5.0).abs() < 1.0, "mid.z={}, expected ~5.0", mid.z);
    }

    #[test]
    fn loft_nurbs_three_sections() {
        let c1 = line_curve(DVec3::new(-5.0, 0.0, 0.0), DVec3::new(5.0, 0.0, 0.0));
        let c2 = line_curve(DVec3::new(-3.0, 0.0, 5.0), DVec3::new(3.0, 0.0, 5.0));
        let c3 = line_curve(DVec3::new(-5.0, 0.0, 10.0), DVec3::new(5.0, 0.0, 10.0));
        let surf = loft_nurbs(&[c1, c2, c3], 8);

        // The surface should exist and be evaluable
        let pt = surf.point_at(0.5, 0.0);
        assert!((pt.z - 0.0).abs() < 1.0, "v=0 should be near first section: z={}", pt.z);
    }

    #[test]
    fn loft_nurbs_endpoint_fidelity() {
        let c1 = nurbs_uniform(vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(5.0, 3.0, 0.0),
            DVec3::new(10.0, 0.0, 0.0),
        ], 2);
        let c2 = nurbs_uniform(vec![
            DVec3::new(0.0, 0.0, 10.0),
            DVec3::new(5.0, 3.0, 10.0),
            DVec3::new(10.0, 0.0, 10.0),
        ], 2);
        let surf = loft_nurbs(&[c1, c2], 10);

        // At v=0 (first section), u=0 should be near (0,0,0)
        let p00 = surf.point_at(0.0, 0.0);
        assert!((p00 - DVec3::new(0.0, 0.0, 0.0)).length() < 1.0,
                "p(0,0) should be near first section start: {p00:?}");
    }

    #[test]
    fn loft_nurbs_guided_basic() {
        let c1 = line_curve(DVec3::new(-5.0, 0.0, 0.0), DVec3::new(5.0, 0.0, 0.0));
        let c2 = line_curve(DVec3::new(-5.0, 0.0, 10.0), DVec3::new(5.0, 0.0, 10.0));

        // Guide that bows outward in Y
        let guide = nurbs_uniform(vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 5.0, 5.0),
            DVec3::new(0.0, 0.0, 10.0),
        ], 2);

        let surf = loft_nurbs_guided(&[c1, c2], &[guide], 10);

        // Without guide, center of loft would be at y=0.
        // With guide bowing to y=5 at midpoint, surface should bow out in Y.
        let mid = surf.point_at(0.5, 0.5);
        assert!(mid.y > 0.5, "guide should pull surface toward positive Y: mid.y={}", mid.y);
    }

    #[test]
    fn loft_nurbs_guided_no_guides_same_as_plain() {
        let c1 = line_curve(DVec3::new(-5.0, 0.0, 0.0), DVec3::new(5.0, 0.0, 0.0));
        let c2 = line_curve(DVec3::new(-5.0, 0.0, 10.0), DVec3::new(5.0, 0.0, 10.0));

        let plain = loft_nurbs(&[c1.clone(), c2.clone()], 8);
        let guided = loft_nurbs_guided(&[c1, c2], &[], 8);

        let p1 = plain.point_at(0.5, 0.5);
        let p2 = guided.point_at(0.5, 0.5);
        assert!((p1 - p2).length() < 1e-6, "no guides should equal plain loft");
    }

    #[test]
    #[should_panic(expected = "loft_nurbs requires at least 2 profiles")]
    fn loft_nurbs_single_profile_panics() {
        let c1 = line_curve(DVec3::ZERO, DVec3::X);
        loft_nurbs(&[c1], 5);
    }
}
