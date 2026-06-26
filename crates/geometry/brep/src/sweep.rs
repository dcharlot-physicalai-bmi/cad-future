//! Sweep operation — move a 2D profile along a 3D path curve to create a solid.
//!
//! The sweep places instances of the profile at evenly spaced sample points
//! along the path, orients each instance using a local Frenet-like frame,
//! and connects adjacent instances with quad faces. Cap faces close the solid
//! at both ends.
//!
//! ## Frenet frame
//! `frenet_frame` computes the exact Frenet-Serret frame (tangent, normal,
//! binormal) at each station along the path, with fallback to a
//! rotation-minimizing frame when curvature vanishes.
//!
//! ## Guide-curve sweep
//! `sweep_guided` uses one or more guide curves to control how the profile
//! scales and orients as it moves along the path.

use glam::{DVec2, DVec3};

use crate::curve::{perpendicular_frame, Curve};
use crate::profile::Profile;
use crate::solid::Solid;
use crate::surface::Surface;
use crate::types::VertexId;

// ---------------------------------------------------------------------------
// Frenet / rotation-minimizing frame
// ---------------------------------------------------------------------------

/// Frenet-Serret frame at a point on a curve.
#[derive(Clone, Debug)]
pub struct FrenetFrame {
    /// Unit tangent (T).
    pub tangent: DVec3,
    /// Unit normal (N) — points toward center of curvature.
    pub normal: DVec3,
    /// Unit binormal (B = T x N).
    pub binormal: DVec3,
}

/// Compute the tangent of `path` at parameter `t` via central finite difference.
pub fn path_tangent(path: &Curve, t: f64) -> DVec3 {
    let eps = 1e-6;
    let t_fwd = (t + eps).min(1.0);
    let t_bwd = (t - eps).max(0.0);
    (path.evaluate(t_fwd) - path.evaluate(t_bwd)).normalize()
}

/// Compute the Frenet-Serret frame (T, N, B) at parameter `t` on a curve.
///
/// When the curvature is near zero (straight segment) the normal and binormal
/// are computed from a perpendicular frame of the tangent.
pub fn frenet_frame(path: &Curve, t: f64) -> FrenetFrame {
    let eps = 1e-5;
    let t0 = (t - eps).max(0.0);
    let t1 = t;
    let t2 = (t + eps).min(1.0);

    let p0 = path.evaluate(t0);
    let p1 = path.evaluate(t1);
    let p2 = path.evaluate(t2);

    let d1 = p1 - p0;
    let d2 = p2 - p1;

    let tangent = ((d1 + d2) * 0.5).normalize();

    // Second derivative approximation
    let dd = d2 - d1;
    let curvature_vec = dd - tangent * dd.dot(tangent);

    if curvature_vec.length() > 1e-10 {
        let normal = curvature_vec.normalize();
        let binormal = tangent.cross(normal).normalize();
        FrenetFrame { tangent, normal, binormal }
    } else {
        // Curvature is zero — use perpendicular_frame
        let (n, b) = perpendicular_frame(tangent);
        FrenetFrame { tangent, normal: n, binormal: b }
    }
}

/// Compute a sequence of rotation-minimizing frames (RMF) along a path.
///
/// RMF avoids the sudden flips that Frenet frames can exhibit at inflection
/// points by propagating the frame from one station to the next using double
/// reflection (the method of Wang et al.).
pub fn rotation_minimizing_frames(path: &Curve, num_stations: usize) -> Vec<FrenetFrame> {
    assert!(num_stations >= 2);

    let mut frames = Vec::with_capacity(num_stations);

    // Seed: Frenet frame at t = 0
    let f0 = frenet_frame(path, 0.0);
    frames.push(f0);

    for i in 1..num_stations {
        let t_prev = (i - 1) as f64 / (num_stations - 1) as f64;
        let t_curr = i as f64 / (num_stations - 1) as f64;

        let x_prev = path.evaluate(t_prev);
        let x_curr = path.evaluate(t_curr);
        let t_curr_vec = path_tangent(path, t_curr);

        let prev = &frames[i - 1];

        // Double reflection method
        let v1 = x_curr - x_prev;
        let c1 = v1.dot(v1);
        if c1 < 1e-20 {
            // Degenerate step, copy previous
            frames.push(prev.clone());
            continue;
        }

        let r_l = prev.normal - v1 * (2.0 / c1) * v1.dot(prev.normal);
        let t_l = prev.tangent - v1 * (2.0 / c1) * v1.dot(prev.tangent);

        let v2 = t_curr_vec - t_l;
        let c2 = v2.dot(v2);

        let normal = if c2 < 1e-20 {
            r_l
        } else {
            r_l - v2 * (2.0 / c2) * v2.dot(r_l)
        };

        let normal = normal.normalize();
        let binormal = t_curr_vec.cross(normal).normalize();

        frames.push(FrenetFrame {
            tangent: t_curr_vec,
            normal,
            binormal,
        });
    }

    frames
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a local frame (normal, binormal) from a tangent vector.
fn local_frame(tangent: DVec3) -> (DVec3, DVec3) {
    perpendicular_frame(tangent)
}

/// Place 2D profile vertices into 3D at a given origin and local frame.
fn place_profile(profile_pts: &[DVec2], origin: DVec3, u: DVec3, v: DVec3) -> Vec<DVec3> {
    profile_pts
        .iter()
        .map(|p| origin + u * p.x + v * p.y)
        .collect()
}


// ---------------------------------------------------------------------------
// Core sweep
// ---------------------------------------------------------------------------

/// Sweep a 2D profile along a 3D path curve to create a solid.
///
/// # Arguments
/// * `profile` -- closed 2D profile to sweep.
/// * `path`    -- 3D curve defining the sweep trajectory (evaluated over [0, 1]).
/// * `steps`   -- number of steps along the path; the path is sampled at `steps + 1` points.
///
/// # Panics
/// Panics if the profile has fewer than 3 vertices or `steps` is zero.
pub fn sweep(profile: &Profile, path: &Curve, steps: usize) -> Solid {
    assert!(steps > 0, "sweep requires at least 1 step");

    let pts_2d = profile.vertices_2d();
    let n = pts_2d.len();
    assert!(n >= 3, "sweep requires a profile with at least 3 vertices");

    let mut solid = Solid::new();

    // ------------------------------------------------------------------
    // 1. Sample the path and place profile vertices at each station
    // ------------------------------------------------------------------
    let num_stations = steps + 1;

    // rings[station][vertex_index] = VertexId
    let mut rings: Vec<Vec<VertexId>> = Vec::with_capacity(num_stations);

    for s in 0..num_stations {
        let t = s as f64 / steps as f64;
        let origin = path.evaluate(t);
        let tangent = path_tangent(path, t);
        let (u, v) = local_frame(tangent);
        let pts_3d = place_profile(&pts_2d, origin, u, v);

        let ids: Vec<VertexId> = pts_3d.into_iter().map(|p| solid.add_vertex(p)).collect();
        rings.push(ids);
    }

    // ------------------------------------------------------------------
    // 2. Start cap (t = 0) -- winding reversed so outward normal opposes path
    // ------------------------------------------------------------------
    {
        let tangent = path_tangent(path, 0.0);
        let cap_normal = -tangent;
        let origin = path.evaluate(0.0);
        let cap_verts: Vec<VertexId> = rings[0].iter().rev().copied().collect();
        solid.add_face_from_vertices(Surface::plane(origin, cap_normal), &cap_verts, true);
    }

    // ------------------------------------------------------------------
    // 3. End cap (t = 1) -- winding follows path direction
    // ------------------------------------------------------------------
    {
        let tangent = path_tangent(path, 1.0);
        let origin = path.evaluate(1.0);
        solid.add_face_from_vertices(Surface::plane(origin, tangent), &rings[steps], true);
    }

    // ------------------------------------------------------------------
    // 4. Side faces -- one quad per (profile edge x path step)
    // ------------------------------------------------------------------
    for s in 0..steps {
        let ring_a = &rings[s];
        let ring_b = &rings[s + 1];

        for i in 0..n {
            let j = (i + 1) % n;

            let v0 = ring_a[i];
            let v1 = ring_a[j];
            let v2 = ring_b[j];
            let v3 = ring_b[i];

            let p0 = solid.vertices[v0].point;
            let p1 = solid.vertices[v1].point;
            let p3 = solid.vertices[v3].point;
            let edge_a = p1 - p0;
            let edge_b = p3 - p0;
            let face_normal = edge_a.cross(edge_b).normalize();

            solid.add_face_from_vertices(
                Surface::plane(p0, face_normal),
                &[v0, v1, v2, v3],
                true,
            );
        }
    }

    // ------------------------------------------------------------------
    // 5. Link twin half-edges across shared boundaries
    // ------------------------------------------------------------------
    solid.link_twins();

    solid
}

// ---------------------------------------------------------------------------
// Sweep with guide curves
// ---------------------------------------------------------------------------

/// Sweep a profile along a path with orientation and scaling controlled by
/// guide curves.
///
/// Each guide curve is sampled at every station; the profile at that station is
/// scaled so that the guide point lies on the profile boundary. When multiple
/// guides are provided their influence is averaged.
///
/// Uses rotation-minimizing frames for twist-free orientation along the path.
///
/// # Arguments
/// * `profile` -- closed 2D profile.
/// * `path`    -- 3D spine curve.
/// * `guides`  -- one or more 3D guide curves (evaluated [0, 1]).
/// * `steps`   -- number of path subdivisions.
pub fn sweep_guided(
    profile: &Profile,
    path: &Curve,
    guides: &[Curve],
    steps: usize,
) -> Solid {
    assert!(steps > 0, "sweep_guided requires at least 1 step");

    let pts_2d = profile.vertices_2d();
    let n = pts_2d.len();
    assert!(n >= 3, "sweep_guided requires a profile with at least 3 vertices");

    let num_stations = steps + 1;

    // Compute rotation-minimizing frames along the path
    let frames = rotation_minimizing_frames(path, num_stations);

    // Compute the profile's bounding radius (max distance from centroid)
    let cx: f64 = pts_2d.iter().map(|p| p.x).sum::<f64>() / n as f64;
    let cy: f64 = pts_2d.iter().map(|p| p.y).sum::<f64>() / n as f64;
    let profile_radius = pts_2d.iter()
        .map(|p| ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt())
        .fold(0.0_f64, f64::max);

    let mut solid = Solid::new();
    let mut rings: Vec<Vec<VertexId>> = Vec::with_capacity(num_stations);

    for s in 0..num_stations {
        let t = s as f64 / steps as f64;
        let origin = path.evaluate(t);
        let frame = &frames[s];

        // Compute scale factor from guide curves
        let mut scale = 1.0;
        if !guides.is_empty() && profile_radius > 1e-12 {
            let mut total_scale = 0.0;
            for guide in guides {
                let guide_pt = guide.evaluate(t);
                let offset = guide_pt - origin;
                let dist = offset.length();
                total_scale += dist / profile_radius;
            }
            scale = total_scale / guides.len() as f64;
            if scale < 1e-6 { scale = 1e-6; }
        }

        // Place profile with scale
        let pts_3d: Vec<DVec3> = pts_2d.iter()
            .map(|p| {
                origin + frame.normal * (p.x * scale) + frame.binormal * (p.y * scale)
            })
            .collect();

        let ids: Vec<VertexId> = pts_3d.into_iter().map(|p| solid.add_vertex(p)).collect();
        rings.push(ids);
    }

    // Caps
    {
        let tangent = path_tangent(path, 0.0);
        let cap_normal = -tangent;
        let origin = path.evaluate(0.0);
        let cap_verts: Vec<VertexId> = rings[0].iter().rev().copied().collect();
        solid.add_face_from_vertices(Surface::plane(origin, cap_normal), &cap_verts, true);
    }
    {
        let tangent = path_tangent(path, 1.0);
        let origin = path.evaluate(1.0);
        solid.add_face_from_vertices(Surface::plane(origin, tangent), &rings[steps], true);
    }

    // Side faces
    for s in 0..steps {
        let ring_a = &rings[s];
        let ring_b = &rings[s + 1];

        for i in 0..n {
            let j = (i + 1) % n;
            let v0 = ring_a[i];
            let v1 = ring_a[j];
            let v2 = ring_b[j];
            let v3 = ring_b[i];

            let p0 = solid.vertices[v0].point;
            let p1 = solid.vertices[v1].point;
            let p3 = solid.vertices[v3].point;
            let edge_a = p1 - p0;
            let edge_b = p3 - p0;
            let face_normal = edge_a.cross(edge_b).normalize();

            solid.add_face_from_vertices(
                Surface::plane(p0, face_normal),
                &[v0, v1, v2, v3],
                true,
            );
        }
    }

    solid.link_twins();
    solid
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::{Curve, nurbs_uniform};
    use crate::profile::Profile;
    use glam::DVec3;

    /// Helper: rectangular profile (4 vertices).
    fn rect_profile() -> Profile {
        Profile::rectangle(2.0, 1.0)
    }

    // -----------------------------------------------------------------
    // 1. sweep_straight_line -- equivalent to an extrusion
    // -----------------------------------------------------------------
    #[test]
    fn sweep_straight_line() {
        let profile = rect_profile();
        let path = Curve::line(DVec3::ZERO, DVec3::new(0.0, 0.0, 10.0));
        let solid = sweep(&profile, &path, 1);

        assert_eq!(solid.vertex_count(), 8);
        assert_eq!(solid.face_count(), 6);
        assert!(solid.is_valid_shell(), "Euler characteristic = {}", solid.euler_characteristic());
    }

    // -----------------------------------------------------------------
    // 2. sweep_along_arc -- sweep along a circular arc path
    // -----------------------------------------------------------------
    #[test]
    fn sweep_along_arc() {
        let profile = Profile::rectangle(0.5, 0.5);
        let path = Curve::arc(
            DVec3::ZERO,
            DVec3::Y,
            5.0,
            0.0,
            std::f64::consts::FRAC_PI_2,
        );
        let steps = 8;
        let solid = sweep(&profile, &path, steps);

        let expected_verts = 4 * (steps + 1);
        assert_eq!(solid.vertex_count(), expected_verts);
        assert!(solid.is_valid_shell(), "Euler characteristic = {}", solid.euler_characteristic());
    }

    // -----------------------------------------------------------------
    // 3. sweep_vertex_count
    // -----------------------------------------------------------------
    #[test]
    fn sweep_vertex_count() {
        let profile = Profile::l_shape(4.0, 6.0, 1.0);
        let path = Curve::line(DVec3::ZERO, DVec3::new(0.0, 0.0, 5.0));
        let steps = 4;
        let solid = sweep(&profile, &path, steps);

        assert_eq!(solid.vertex_count(), 6 * (steps + 1));
    }

    // -----------------------------------------------------------------
    // 4. sweep_valid_shell
    // -----------------------------------------------------------------
    #[test]
    fn sweep_valid_shell() {
        let profile = Profile::l_shape(4.0, 6.0, 1.0);
        let path = Curve::line(DVec3::ZERO, DVec3::new(10.0, 0.0, 10.0));
        let steps = 3;
        let solid = sweep(&profile, &path, steps);

        assert_eq!(
            solid.euler_characteristic(),
            2,
            "V={} E={} F={} => chi={}",
            solid.vertex_count(),
            solid.edge_count(),
            solid.face_count(),
            solid.euler_characteristic(),
        );
    }

    // -----------------------------------------------------------------
    // Frenet frame tests
    // -----------------------------------------------------------------

    #[test]
    fn frenet_frame_straight_line() {
        let path = Curve::line(DVec3::ZERO, DVec3::new(0.0, 0.0, 10.0));
        let f = frenet_frame(&path, 0.5);
        // Tangent should be along Z
        assert!(f.tangent.z.abs() > 0.99, "tangent should be along Z: {:?}", f.tangent);
        // Normal and binormal should be perpendicular to tangent
        assert!(f.tangent.dot(f.normal).abs() < 1e-6);
        assert!(f.tangent.dot(f.binormal).abs() < 1e-6);
        assert!(f.normal.dot(f.binormal).abs() < 1e-6);
    }

    #[test]
    fn frenet_frame_circular_arc() {
        let path = Curve::arc(DVec3::ZERO, DVec3::Z, 5.0, 0.0, std::f64::consts::FRAC_PI_2);
        let f = frenet_frame(&path, 0.5);
        // Frame should be orthonormal
        assert!((f.tangent.length() - 1.0).abs() < 1e-6);
        assert!((f.normal.length() - 1.0).abs() < 1e-6);
        assert!((f.binormal.length() - 1.0).abs() < 1e-6);
        assert!(f.tangent.dot(f.normal).abs() < 1e-4);
    }

    #[test]
    fn rotation_minimizing_frames_orthonormal() {
        let path = nurbs_uniform(vec![
            DVec3::ZERO,
            DVec3::new(5.0, 3.0, 2.0),
            DVec3::new(10.0, 0.0, 5.0),
        ], 2);
        let frames = rotation_minimizing_frames(&path, 10);
        assert_eq!(frames.len(), 10);
        for f in &frames {
            assert!((f.tangent.length() - 1.0).abs() < 1e-4, "tangent not unit: {}", f.tangent.length());
            assert!((f.normal.length() - 1.0).abs() < 1e-4, "normal not unit: {}", f.normal.length());
            assert!(f.tangent.dot(f.normal).abs() < 1e-3, "T.N not zero: {}", f.tangent.dot(f.normal));
        }
    }

    // -----------------------------------------------------------------
    // Guided sweep tests
    // -----------------------------------------------------------------

    #[test]
    fn sweep_guided_basic() {
        let profile = Profile::rectangle(2.0, 2.0);
        let path = Curve::line(DVec3::ZERO, DVec3::new(0.0, 0.0, 10.0));

        // Guide that moves away from path -> profile should scale up
        let guide = Curve::line(
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(4.0, 0.0, 10.0),
        );

        let solid = sweep_guided(&profile, &path, &[guide], 4);
        assert!(solid.is_valid_shell(), "chi={}", solid.euler_characteristic());

        // The end ring should be wider than the start ring
        let (_min, max) = solid.bounding_box();
        // At z=10, the profile should be scaled up
        assert!(max.x > 1.5, "max.x should be > 1.5 for growing guide: {}", max.x);
    }

    #[test]
    fn sweep_guided_valid_shell() {
        let profile = Profile::rectangle(1.0, 1.0);
        let path = Curve::line(DVec3::ZERO, DVec3::new(0.0, 0.0, 5.0));
        let guide = Curve::line(DVec3::new(1.0, 0.0, 0.0), DVec3::new(1.0, 0.0, 5.0));

        let solid = sweep_guided(&profile, &path, &[guide], 3);
        assert_eq!(solid.euler_characteristic(), 2);
    }

    #[test]
    fn sweep_guided_no_guides_same_topology() {
        let profile = Profile::rectangle(2.0, 1.0);
        let path = Curve::line(DVec3::ZERO, DVec3::new(0.0, 0.0, 10.0));
        let steps = 2;

        let plain = sweep(&profile, &path, steps);
        let guided = sweep_guided(&profile, &path, &[], steps);

        assert_eq!(plain.vertex_count(), guided.vertex_count());
        assert_eq!(plain.face_count(), guided.face_count());
    }
}
