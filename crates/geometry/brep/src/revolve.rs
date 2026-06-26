//! Revolve operation — create a solid of revolution from a 2D profile.
//!
//! Rotates a closed 2D profile around an axis to produce a solid (cups,
//! shafts, pulleys, etc.).  Supports both full 360° revolutions and partial
//! sweeps with planar cap faces.

use std::f64::consts::TAU;

use glam::DVec3;

use crate::profile::Profile;
use crate::solid::Solid;
use crate::surface::Surface;
use crate::types::VertexId;

// ---------------------------------------------------------------------------
// Rodrigues rotation
// ---------------------------------------------------------------------------

/// Rotate `point` around `axis` (through the origin) by `angle` radians
/// using the Rodrigues rotation formula.
fn rodrigues(point: DVec3, axis: DVec3, angle: f64) -> DVec3 {
    let k = axis.normalize();
    let (sin_a, cos_a) = angle.sin_cos();
    point * cos_a + k.cross(point) * sin_a + k * k.dot(point) * (1.0 - cos_a)
}

/// True when the revolution angle covers a full circle (within tolerance).
fn is_full_revolution(angle: f64) -> bool {
    (angle.abs() - TAU).abs() < 1e-10
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Create a solid of revolution by rotating `profile` around an axis.
///
/// # Parameters
/// - `profile`  — closed 2D profile to revolve
/// - `origin`   — a point on the rotation axis
/// - `axis`     — rotation axis direction (will be normalised internally)
/// - `u`        — profile U axis (radial direction at angle = 0)
/// - `v`        — profile V axis (along the rotation axis)
/// - `angle`    — revolution angle in radians (`TAU` for full revolution)
/// - `segments` — number of angular segments
pub fn revolve(
    profile: &Profile,
    origin: DVec3,
    axis: DVec3,
    u: DVec3,
    v: DVec3,
    angle: f64,
    segments: usize,
) -> Solid {
    let mut solid = Solid::new();

    let pts_2d = profile.vertices_2d();
    let n_pts = pts_2d.len();
    if n_pts < 2 || segments == 0 {
        return solid;
    }

    let full = is_full_revolution(angle);
    // For a full revolution the last ring wraps back to the first, so we only
    // need `segments` unique rings.  For partial revolutions we need
    // `segments + 1` rings (start ring + end ring).
    let n_rings = if full { segments } else { segments + 1 };
    let step = angle / segments as f64;

    // ------------------------------------------------------------------
    // 1. Create vertex rings
    // ------------------------------------------------------------------
    // `rings[ring][pt]` = VertexId
    let mut rings: Vec<Vec<VertexId>> = Vec::with_capacity(n_rings);

    for ring_idx in 0..n_rings {
        let theta = step * ring_idx as f64;
        let mut ring = Vec::with_capacity(n_pts);
        for p2 in &pts_2d {
            // Lift 2D → 3D in the profile plane, then rotate around axis
            let p3 = origin + u * p2.x + v * p2.y;
            let offset = p3 - origin;
            let rotated = origin + rodrigues(offset, axis, theta);
            ring.push(solid.add_vertex(rotated));
        }
        rings.push(ring);
    }

    // ------------------------------------------------------------------
    // 2. Side (lateral) faces — quads connecting adjacent rings
    // ------------------------------------------------------------------
    let axis_n = axis.normalize();

    for seg in 0..segments {
        let ring_a = seg;
        let ring_b = if full { (seg + 1) % n_rings } else { seg + 1 };

        for pt in 0..n_pts {
            let pt_next = (pt + 1) % n_pts;

            // Four corners of the quad (CCW when viewed from outside)
            let v0 = rings[ring_a][pt];
            let v1 = rings[ring_a][pt_next];
            let v2 = rings[ring_b][pt_next];
            let v3 = rings[ring_b][pt];

            // Choose surface type: if the two profile points are at the same
            // distance from the axis the face lies on a cylinder (or is
            // degenerate for radius ≈ 0).  Otherwise fall back to a plane
            // approximation.
            let surface = classify_side_surface(
                &solid, v0, v1, v2, v3, origin, axis_n, &pts_2d, pt, pt_next, &u,
            );

            solid.add_face_from_vertices(surface, &[v0, v1, v2, v3], true);
        }
    }

    // ------------------------------------------------------------------
    // 3. Cap faces (partial revolution only)
    // ------------------------------------------------------------------
    if !full {
        // Start cap — the profile at angle = 0
        let start_cap: Vec<VertexId> = rings[0].iter().rev().copied().collect();
        let start_normal = -rodrigues(u.cross(v).normalize(), axis, 0.0);
        // The cap plane passes through the profile at this angle
        let start_origin = origin + u * pts_2d[0].x + v * pts_2d[0].y;
        solid.add_face_from_vertices(
            Surface::plane(start_origin, start_normal),
            &start_cap,
            true,
        );

        // End cap — the profile at the final angle
        let end_cap: Vec<VertexId> = rings[n_rings - 1].clone();
        let end_normal = rodrigues(u.cross(v).normalize(), axis, angle);
        let end_origin_pt = {
            let p2 = pts_2d[0];
            let p3 = origin + u * p2.x + v * p2.y;
            let offset = p3 - origin;
            origin + rodrigues(offset, axis, angle)
        };
        solid.add_face_from_vertices(
            Surface::plane(end_origin_pt, end_normal),
            &end_cap,
            true,
        );
    }

    solid.link_twins();
    solid
}

/// Full 360° revolution convenience wrapper.
pub fn revolve_full(
    profile: &Profile,
    origin: DVec3,
    axis: DVec3,
    u: DVec3,
    v: DVec3,
    segments: usize,
) -> Solid {
    revolve(profile, origin, axis, u, v, TAU, segments)
}

// ---------------------------------------------------------------------------
// Surface classification helper
// ---------------------------------------------------------------------------

/// Pick the best `Surface` for a lateral quad.
fn classify_side_surface(
    solid: &Solid,
    v0: VertexId,
    v1: VertexId,
    v2: VertexId,
    v3: VertexId,
    origin: DVec3,
    axis: DVec3,
    pts_2d: &[glam::DVec2],
    pt: usize,
    pt_next: usize,
    _u_axis: &DVec3,
) -> Surface {
    // Radial distance of the two profile points from the rotation axis.
    // In the profile plane, the radial component is the projection onto `u`.
    let r0 = pts_2d[pt].x;
    let r1 = pts_2d[pt_next].x;

    if (r0 - r1).abs() < 1e-10 {
        // Constant radius → cylindrical surface
        let radius = r0.abs();
        if radius < 1e-12 {
            // Degenerate — on the axis.  Use a plane as a fallback.
            let p0 = solid.vertices[v0].point;
            let p1 = solid.vertices[v1].point;
            let p2 = solid.vertices[v2].point;
            let normal = (p1 - p0).cross(p2 - p0).normalize();
            Surface::plane(p0, normal)
        } else {
            Surface::cylinder(origin, axis, radius)
        }
    } else {
        // Varying radius — use a plane through the quad.
        let p0 = solid.vertices[v0].point;
        let p1 = solid.vertices[v1].point;
        let p2 = solid.vertices[v2].point;
        let normal = (p1 - p0).cross(p2 - p0);
        let len = normal.length();
        if len < 1e-14 {
            // Fallback: try another triangle of the quad
            let p3 = solid.vertices[v3].point;
            let alt_normal = (p2 - p0).cross(p3 - p0).normalize();
            Surface::plane(p0, alt_normal)
        } else {
            Surface::plane(p0, normal / len)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Profile, ProfileSegment};
    use glam::DVec2;
    use std::f64::consts::{FRAC_PI_2, TAU};

    /// Helper: a rectangle profile sitting at radial offset `x_offset` from
    /// the axis, with width `w` (radial) and height `h` (axial).
    fn rect_profile(x_offset: f64, w: f64, h: f64) -> Profile {
        let x0 = x_offset;
        let x1 = x_offset + w;
        let y0 = -h / 2.0;
        let y1 = h / 2.0;
        Profile::new(vec![
            ProfileSegment::line(DVec2::new(x0, y0), DVec2::new(x1, y0)),
            ProfileSegment::line(DVec2::new(x1, y0), DVec2::new(x1, y1)),
            ProfileSegment::line(DVec2::new(x1, y1), DVec2::new(x0, y1)),
            ProfileSegment::line(DVec2::new(x0, y1), DVec2::new(x0, y0)),
        ])
    }

    #[test]
    fn revolve_full_cylinder() {
        // Revolve a thin rectangle at radius 5 around Y to form a hollow
        // cylinder-like shell.
        let profile = rect_profile(4.0, 2.0, 10.0);
        let solid = revolve_full(&profile, DVec3::ZERO, DVec3::Y, DVec3::X, DVec3::Y, 16);

        // Full revolution, polygonal profile with 4 pts, 16 segments:
        //   V = 4 * 16 = 64
        //   F = 4 * 16 = 64  (all side quads, no caps)
        //   E = V + F - 2 = 126  (Euler for closed genus-1 → V-E+F = 0)
        // Actually for a torus topology (genus 1): V - E + F = 0
        // V = 64, F = 64, E = 128 → 64 - 128 + 64 = 0 ✓
        assert_eq!(solid.vertex_count(), 64);
        assert_eq!(solid.face_count(), 64);
        assert_eq!(solid.edge_count(), 128);
        // Genus-1 torus topology → Euler characteristic = 0
        assert_eq!(solid.euler_characteristic(), 0);
    }

    #[test]
    fn revolve_full_torus() {
        // Revolve a small circle-approximation (octagon) at offset from axis
        // to get a torus-like shape.
        let n = 8;
        let r = 1.0;
        let cx = 5.0; // offset from axis
        let mut segs = Vec::new();
        for i in 0..n {
            let a0 = TAU * i as f64 / n as f64;
            let a1 = TAU * (i + 1) as f64 / n as f64;
            let p0 = DVec2::new(cx + r * a0.cos(), r * a0.sin());
            let p1 = DVec2::new(cx + r * a1.cos(), r * a1.sin());
            segs.push(ProfileSegment::line(p0, p1));
        }
        let profile = Profile::new(segs);
        let solid = revolve_full(&profile, DVec3::ZERO, DVec3::Y, DVec3::X, DVec3::Y, 16);

        // 8 profile pts × 16 segments = 128 vertices
        assert_eq!(solid.vertex_count(), 128);
        // 8 × 16 = 128 faces (all quads)
        assert_eq!(solid.face_count(), 128);
        // Torus topology → Euler characteristic 0
        assert_eq!(solid.euler_characteristic(), 0);
    }

    #[test]
    fn revolve_quarter_has_caps() {
        let profile = rect_profile(3.0, 2.0, 6.0);
        let solid = revolve(
            &profile,
            DVec3::ZERO,
            DVec3::Y,
            DVec3::X,
            DVec3::Y,
            FRAC_PI_2, // 90°
            8,
        );

        // Partial revolution: 4 pts × 9 rings = 36 vertices
        // Side faces: 4 × 8 = 32
        // Cap faces: 2
        // Total faces: 34
        assert_eq!(solid.vertex_count(), 36);
        assert_eq!(solid.face_count(), 34);
        // Should be a valid closed shell (Euler = 2)
        assert!(solid.is_valid_shell(), "quarter revolve should produce a valid closed shell");
    }

    #[test]
    fn revolve_bounding_box() {
        // Revolve a rectangle [4..6] × [-5..5] around Y with full revolution.
        // Expected bounding box: x,z in [-6, 6], y in [-5, 5].
        let profile = rect_profile(4.0, 2.0, 10.0);
        let solid = revolve_full(&profile, DVec3::ZERO, DVec3::Y, DVec3::X, DVec3::Y, 32);

        let (min, max) = solid.bounding_box();
        let tol = 0.15; // polygonal approximation tolerance for 32 segments

        assert!((min.y - (-5.0)).abs() < 1e-10, "min.y = {}", min.y);
        assert!((max.y - 5.0).abs() < 1e-10, "max.y = {}", max.y);
        // Outer radius = 6, inner = 4.  Bounding box extremes at ±6.
        assert!((max.x - 6.0).abs() < tol, "max.x = {}", max.x);
        assert!((min.x - (-6.0)).abs() < tol, "min.x = {}", min.x);
        assert!((max.z - 6.0).abs() < tol, "max.z = {}", max.z);
        assert!((min.z - (-6.0)).abs() < tol, "min.z = {}", min.z);
    }

    #[test]
    fn revolve_vertex_count() {
        let profile = rect_profile(5.0, 3.0, 8.0);

        // Full revolution: V = n_pts × segments
        let full = revolve_full(&profile, DVec3::ZERO, DVec3::Y, DVec3::X, DVec3::Y, 20);
        assert_eq!(full.vertex_count(), 4 * 20);

        // Partial revolution: V = n_pts × (segments + 1)
        let partial = revolve(
            &profile,
            DVec3::ZERO,
            DVec3::Y,
            DVec3::X,
            DVec3::Y,
            std::f64::consts::PI,
            12,
        );
        assert_eq!(partial.vertex_count(), 4 * 13);
    }
}
