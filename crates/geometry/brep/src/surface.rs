//! Surface types for B-Rep faces.

use glam::DVec3;
use serde::{Serialize, Deserialize};
use crate::curve::{bspline_basis, Curve};

/// A surface used as the geometric support for a face.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Surface {
    /// Infinite plane defined by point and normal.
    Plane { origin: DVec3, normal: DVec3 },
    /// Cylinder along an axis.
    Cylinder { origin: DVec3, axis: DVec3, radius: f64 },
    /// Sphere.
    Sphere { center: DVec3, radius: f64 },
    /// Torus (for fillet blends at convex edges).
    Torus { center: DVec3, axis: DVec3, major_radius: f64, minor_radius: f64 },
    /// Cone.
    Cone { apex: DVec3, axis: DVec3, half_angle: f64 },
    /// NURBS surface (tensor product).
    Nurbs {
        /// Control points grid (rows x cols).
        control_points: Vec<Vec<DVec3>>,
        /// Weights grid.
        weights: Vec<Vec<f64>>,
        /// Knot vector in U direction.
        knots_u: Vec<f64>,
        /// Knot vector in V direction.
        knots_v: Vec<f64>,
        /// Degree in U direction.
        degree_u: usize,
        /// Degree in V direction.
        degree_v: usize,
    },
}

impl Surface {
    pub fn plane(origin: DVec3, normal: DVec3) -> Self {
        Self::Plane { origin, normal: normal.normalize() }
    }

    pub fn cylinder(origin: DVec3, axis: DVec3, radius: f64) -> Self {
        Self::Cylinder { origin, axis: axis.normalize(), radius }
    }

    pub fn sphere(center: DVec3, radius: f64) -> Self {
        Self::Sphere { center, radius }
    }

    /// Evaluate the surface point at parameters (u, v).
    ///
    /// For analytic surfaces the mapping is:
    /// - Plane: origin + u * e1 + v * e2
    /// - Cylinder: origin + v * axis  +  radius * (cos(u) * e1 + sin(u) * e2)
    /// - Sphere: center + radius * (cos(v)*cos(u), cos(v)*sin(u), sin(v))
    /// - NURBS: tensor-product evaluation
    pub fn point_at(&self, u: f64, v: f64) -> DVec3 {
        match self {
            Self::Plane { origin, normal } => {
                let (e1, e2) = plane_frame(*normal);
                *origin + e1 * u + e2 * v
            }
            Self::Cylinder { origin, axis, radius } => {
                let (e1, e2) = crate::curve::perpendicular_frame(*axis);
                *origin + *axis * v + e1 * (radius * u.cos()) + e2 * (radius * u.sin())
            }
            Self::Sphere { center, radius } => {
                // u = longitude [0, 2pi], v = latitude [-pi/2, pi/2]
                *center + DVec3::new(
                    radius * v.cos() * u.cos(),
                    radius * v.cos() * u.sin(),
                    radius * v.sin(),
                )
            }
            Self::Torus { center, axis, major_radius, minor_radius } => {
                let (e1, e2) = crate::curve::perpendicular_frame(*axis);
                let r = *major_radius + *minor_radius * v.cos();
                let circle_pt = e1 * (r * u.cos()) + e2 * (r * u.sin());
                *center + circle_pt + *axis * (*minor_radius * v.sin())
            }
            Self::Cone { apex, axis, half_angle } => {
                let (e1, e2) = crate::curve::perpendicular_frame(*axis);
                let r = v * half_angle.tan();
                *apex + *axis * v + e1 * (r * u.cos()) + e2 * (r * u.sin())
            }
            Self::Nurbs { control_points, weights, knots_u, knots_v, degree_u, degree_v } => {
                nurbs_surface_point(control_points, weights, knots_u, knots_v, *degree_u, *degree_v, u, v)
            }
        }
    }

    /// Compute partial derivative dS/du at (u, v) via central finite differences.
    pub fn partial_du(&self, u: f64, v: f64) -> DVec3 {
        let eps = 1e-7;
        let p_fwd = self.point_at(u + eps, v);
        let p_bwd = self.point_at(u - eps, v);
        (p_fwd - p_bwd) / (2.0 * eps)
    }

    /// Compute partial derivative dS/dv at (u, v) via central finite differences.
    pub fn partial_dv(&self, u: f64, v: f64) -> DVec3 {
        let eps = 1e-7;
        let p_fwd = self.point_at(u, v + eps);
        let p_bwd = self.point_at(u, v - eps);
        (p_fwd - p_bwd) / (2.0 * eps)
    }

    /// Surface normal at parametric coordinates (u, v), computed as cross product
    /// of partial derivatives.
    pub fn normal_at_uv(&self, u: f64, v: f64) -> DVec3 {
        let du = self.partial_du(u, v);
        let dv = self.partial_dv(u, v);
        let n = du.cross(dv);
        if n.length() < 1e-14 { DVec3::Y } else { n.normalize() }
    }

    /// Evaluate the outward normal at a given 3D point on the surface.
    pub fn normal_at(&self, point: DVec3) -> DVec3 {
        match self {
            Self::Plane { normal, .. } => normal.normalize(),
            Self::Cylinder { origin, axis, .. } => {
                let a = axis.normalize();
                let v = point - *origin;
                let proj = v - a * v.dot(a);
                proj.normalize()
            }
            Self::Sphere { center, .. } => {
                (point - *center).normalize()
            }
            Self::Torus { center, axis, major_radius, .. } => {
                let a = axis.normalize();
                let v = point - *center;
                let proj = v - a * v.dot(a);
                let ring_center = *center + proj.normalize() * *major_radius;
                (point - ring_center).normalize()
            }
            Self::Cone { apex, axis, .. } => {
                let a = axis.normalize();
                let v = point - *apex;
                let along = v.dot(a);
                let radial = v - a * along;
                if radial.length() < 1e-12 { return a; }
                let r_norm = radial.normalize();
                let half_angle = match self {
                    Self::Cone { half_angle, .. } => *half_angle,
                    _ => unreachable!(),
                };
                (r_norm * half_angle.cos() - a * half_angle.sin()).normalize()
            }
            Self::Nurbs { control_points, weights, knots_u, knots_v, degree_u, degree_v } => {
                // Use closest_point to find (u,v) then compute normal analytically
                let (u, v, _) = nurbs_closest_point(
                    control_points, weights, knots_u, knots_v, *degree_u, *degree_v, point,
                );
                self.normal_at_uv(u, v)
            }
        }
    }

    /// Return a surface with the normal direction flipped.
    pub fn flipped(&self) -> Self {
        match self {
            Self::Plane { origin, normal } => Self::Plane { origin: *origin, normal: -*normal },
            Self::Cylinder { origin, axis, radius } => Self::Cylinder { origin: *origin, axis: -*axis, radius: *radius },
            Self::Sphere { center, radius } => Self::Sphere { center: *center, radius: *radius },
            Self::Torus { center, axis, major_radius, minor_radius } => Self::Torus { center: *center, axis: -*axis, major_radius: *major_radius, minor_radius: *minor_radius },
            Self::Cone { apex, axis, half_angle } => Self::Cone { apex: *apex, axis: -*axis, half_angle: *half_angle },
            Self::Nurbs { control_points, weights, knots_u, knots_v, degree_u, degree_v } => {
                // Flip by reversing the U direction (reverses normal)
                let mut flipped_cp = control_points.clone();
                flipped_cp.reverse();
                let mut flipped_w = weights.clone();
                flipped_w.reverse();
                Self::Nurbs {
                    control_points: flipped_cp,
                    weights: flipped_w,
                    knots_u: knots_u.clone(),
                    knots_v: knots_v.clone(),
                    degree_u: *degree_u,
                    degree_v: *degree_v,
                }
            }
        }
    }

    /// Signed distance from point to surface (positive = outside).
    pub fn signed_distance(&self, point: DVec3) -> f64 {
        match self {
            Self::Plane { origin, normal } => (point - *origin).dot(*normal),
            Self::Cylinder { origin, axis, radius } => {
                let a = axis.normalize();
                let v = point - *origin;
                let proj = v - a * v.dot(a);
                proj.length() - radius
            }
            Self::Sphere { center, radius } => {
                (point - *center).length() - radius
            }
            _ => 0.0, // Torus/Cone/Nurbs: approximate only
        }
    }

    /// Find the closest point on this surface to a query point.
    /// Returns `(u, v, closest_point)`.
    pub fn closest_point(&self, query: DVec3) -> (f64, f64, DVec3) {
        match self {
            Self::Plane { origin, normal } => {
                let (e1, e2) = plane_frame(*normal);
                let d = query - *origin;
                let u = d.dot(e1);
                let v = d.dot(e2);
                let pt = *origin + e1 * u + e2 * v;
                (u, v, pt)
            }
            Self::Nurbs { control_points, weights, knots_u, knots_v, degree_u, degree_v } => {
                nurbs_closest_point(
                    control_points, weights, knots_u, knots_v,
                    *degree_u, *degree_v, query,
                )
            }
            _ => {
                // For other analytic surfaces, use a grid search + refinement
                analytic_closest_point(self, query)
            }
        }
    }
}

/// Build an orthonormal frame from a normal vector (for planes).
fn plane_frame(normal: DVec3) -> (DVec3, DVec3) {
    crate::curve::perpendicular_frame(normal)
}

// ---------------------------------------------------------------------------
// NURBS surface evaluation
// ---------------------------------------------------------------------------

/// Evaluate a NURBS surface point at parameters (u, v).
pub fn nurbs_surface_point(
    control_points: &[Vec<DVec3>],
    weights: &[Vec<f64>],
    knots_u: &[f64],
    knots_v: &[f64],
    degree_u: usize,
    degree_v: usize,
    u: f64,
    v: f64,
) -> DVec3 {
    let rows = control_points.len();
    if rows == 0 { return DVec3::ZERO; }
    let cols = control_points[0].len();

    let u = u.clamp(knots_u[degree_u], knots_u[rows] - 1e-14);
    let v = v.clamp(knots_v[degree_v], knots_v[cols] - 1e-14);

    let mut numerator = DVec3::ZERO;
    let mut denominator = 0.0;

    for i in 0..rows {
        let nu = bspline_basis(i, degree_u, u, knots_u);
        for j in 0..cols {
            let nv = bspline_basis(j, degree_v, v, knots_v);
            let w = weights[i][j];
            let basis = nu * nv * w;
            numerator += control_points[i][j] * basis;
            denominator += basis;
        }
    }

    if denominator.abs() > 1e-14 { numerator / denominator } else { control_points[0][0] }
}

// ---------------------------------------------------------------------------
// NURBS closest point via Newton iteration
// ---------------------------------------------------------------------------

/// Find the closest (u, v) parameters on a NURBS surface to a query point
/// using grid sampling followed by Newton-Raphson iteration.
///
/// Returns `(u, v, closest_3d_point)`.
pub fn nurbs_closest_point(
    control_points: &[Vec<DVec3>],
    weights: &[Vec<f64>],
    knots_u: &[f64],
    knots_v: &[f64],
    degree_u: usize,
    degree_v: usize,
    query: DVec3,
) -> (f64, f64, DVec3) {
    let rows = control_points.len();
    let cols = control_points[0].len();
    let u_min = knots_u[degree_u];
    let u_max = knots_u[rows] - 1e-14;
    let v_min = knots_v[degree_v];
    let v_max = knots_v[cols] - 1e-14;

    // Phase 1: coarse grid search for good starting (u, v)
    let grid = 10;
    let mut best_u = (u_min + u_max) * 0.5;
    let mut best_v = (v_min + v_max) * 0.5;
    let mut best_dist = f64::MAX;

    for i in 0..=grid {
        let u = u_min + (u_max - u_min) * i as f64 / grid as f64;
        for j in 0..=grid {
            let v = v_min + (v_max - v_min) * j as f64 / grid as f64;
            let pt = nurbs_surface_point(control_points, weights, knots_u, knots_v, degree_u, degree_v, u, v);
            let d = (pt - query).length_squared();
            if d < best_dist {
                best_dist = d;
                best_u = u;
                best_v = v;
            }
        }
    }

    // Phase 2: Newton iteration to refine
    let eps = 1e-7;
    let max_iter = 20;
    let mut u = best_u;
    let mut v = best_v;

    for _ in 0..max_iter {
        let pt = nurbs_surface_point(control_points, weights, knots_u, knots_v, degree_u, degree_v, u, v);
        let diff = pt - query;

        if diff.length() < 1e-12 {
            break;
        }

        // Partial derivatives via finite differences
        let du = (nurbs_surface_point(control_points, weights, knots_u, knots_v, degree_u, degree_v, u + eps, v)
                - nurbs_surface_point(control_points, weights, knots_u, knots_v, degree_u, degree_v, u - eps, v))
                / (2.0 * eps);
        let dv = (nurbs_surface_point(control_points, weights, knots_u, knots_v, degree_u, degree_v, u, v + eps)
                - nurbs_surface_point(control_points, weights, knots_u, knots_v, degree_u, degree_v, u, v - eps))
                / (2.0 * eps);

        // Build 2x2 system:  J^T J [du dv]^T = -J^T diff
        // where J = [du | dv] as columns
        let a11 = du.dot(du);
        let a12 = du.dot(dv);
        let a22 = dv.dot(dv);
        let b1 = -diff.dot(du);
        let b2 = -diff.dot(dv);

        let det = a11 * a22 - a12 * a12;
        if det.abs() < 1e-20 {
            break;
        }

        let delta_u = (a22 * b1 - a12 * b2) / det;
        let delta_v = (a11 * b2 - a12 * b1) / det;

        u = (u + delta_u).clamp(u_min, u_max);
        v = (v + delta_v).clamp(v_min, v_max);

        if delta_u.abs() < 1e-12 && delta_v.abs() < 1e-12 {
            break;
        }
    }

    let pt = nurbs_surface_point(control_points, weights, knots_u, knots_v, degree_u, degree_v, u, v);
    (u, v, pt)
}

// ---------------------------------------------------------------------------
// Surface-surface intersection via marching method
// ---------------------------------------------------------------------------

/// Intersect two surfaces, returning the intersection as a list of NURBS curves.
///
/// Uses a marching method: find seed points on a grid where both surfaces are
/// close, then trace intersection curves by stepping along the tangent direction
/// (cross product of the two surface normals).
pub fn intersect_surfaces(s1: &Surface, s2: &Surface, tol: f64) -> Vec<Curve> {
    // We sample both surfaces on a grid and look for points where signed
    // distance to the other surface is near zero, then march along the
    // intersection curve.

    let grid = 20;
    let step_size = tol * 5.0;
    let max_march_steps = 500;

    // Collect seed points: points on s1 that lie close to s2
    let mut seeds: Vec<(f64, f64)> = Vec::new();
    let (u_range, v_range) = surface_param_range(s1);

    for i in 0..=grid {
        let u = u_range.0 + (u_range.1 - u_range.0) * i as f64 / grid as f64;
        for j in 0..=grid {
            let v = v_range.0 + (v_range.1 - v_range.0) * j as f64 / grid as f64;
            let pt = s1.point_at(u, v);
            let dist = s2.signed_distance(pt).abs();
            if dist < tol * 2.0 {
                seeds.push((u, v));
            }
        }
    }

    if seeds.is_empty() {
        return Vec::new();
    }

    // Deduplicate seeds that are close in parameter space
    let mut unique_seeds: Vec<(f64, f64)> = Vec::new();
    for seed in &seeds {
        let dominated = unique_seeds.iter().any(|s| {
            (s.0 - seed.0).abs() < (u_range.1 - u_range.0) / grid as f64 * 1.5
            && (s.1 - seed.1).abs() < (v_range.1 - v_range.0) / grid as f64 * 1.5
        });
        if !dominated {
            unique_seeds.push(*seed);
        }
    }

    let mut result_curves: Vec<Curve> = Vec::new();

    for &(seed_u, seed_v) in &unique_seeds {
        // Refine seed to lie exactly on intersection
        let seed_pt = s1.point_at(seed_u, seed_v);
        let refined = refine_intersection_point(s1, s2, seed_pt, tol);

        // March in both directions from seed
        let mut fwd_pts = march_intersection(s1, s2, refined, 1.0, step_size, max_march_steps, tol);
        let bwd_pts = march_intersection(s1, s2, refined, -1.0, step_size, max_march_steps, tol);

        // Combine: reversed backward + forward
        let mut all_pts: Vec<DVec3> = bwd_pts.into_iter().rev().collect();
        if !all_pts.is_empty() && !fwd_pts.is_empty() {
            // Avoid duplicating the seed point
            all_pts.pop();
        }
        all_pts.append(&mut fwd_pts);

        if all_pts.len() < 2 {
            continue;
        }

        // Check if this curve is too close to an already found one
        let dominated = result_curves.iter().any(|existing| {
            let mid = existing.evaluate(0.5);
            all_pts.iter().any(|p| (*p - mid).length() < tol * 3.0)
        });
        if dominated {
            continue;
        }

        // Fit a NURBS curve through the intersection points
        let curve = fit_nurbs_through_points(&all_pts, 3.min(all_pts.len() - 1));
        result_curves.push(curve);
    }

    result_curves
}

/// Refine a point to lie on the intersection of two surfaces using projection.
fn refine_intersection_point(s1: &Surface, s2: &Surface, start: DVec3, tol: f64) -> DVec3 {
    let mut pt = start;
    for _ in 0..10 {
        // Project onto s2
        let (_, _, p2) = s2.closest_point(pt);
        // Project back onto s1
        let (_, _, p1) = s1.closest_point(p2);
        let dist = (p1 - p2).length();
        pt = (p1 + p2) * 0.5;
        if dist < tol {
            break;
        }
    }
    pt
}

/// March along the intersection curve in one direction.
fn march_intersection(
    s1: &Surface,
    s2: &Surface,
    start: DVec3,
    direction_sign: f64,
    step_size: f64,
    max_steps: usize,
    tol: f64,
) -> Vec<DVec3> {
    let mut pts = vec![start];
    let mut current = start;

    for _ in 0..max_steps {
        // Compute marching direction: cross product of the two normals
        let n1 = s1.normal_at(current);
        let n2 = s2.normal_at(current);
        let tangent = n1.cross(n2);
        if tangent.length() < 1e-12 {
            break; // Surfaces are tangent, stop
        }
        let tangent = tangent.normalize() * direction_sign;

        // Step along tangent
        let candidate = current + tangent * step_size;

        // Project candidate back onto intersection
        let refined = refine_intersection_point(s1, s2, candidate, tol);

        // Check if we've moved enough
        if (refined - current).length() < tol * 0.1 {
            break;
        }

        // Check if we've looped back
        if pts.len() > 2 && (refined - pts[0]).length() < step_size * 2.0 {
            pts.push(pts[0]); // Close the loop
            break;
        }

        current = refined;
        pts.push(current);
    }

    pts
}

/// Return a reasonable parameter range for a surface.
fn surface_param_range(s: &Surface) -> ((f64, f64), (f64, f64)) {
    match s {
        Surface::Plane { .. } => ((-10.0, 10.0), (-10.0, 10.0)),
        Surface::Cylinder { .. } => ((0.0, std::f64::consts::TAU), (-10.0, 10.0)),
        Surface::Sphere { .. } => ((0.0, std::f64::consts::TAU), (-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2)),
        Surface::Torus { .. } => ((0.0, std::f64::consts::TAU), (0.0, std::f64::consts::TAU)),
        Surface::Cone { .. } => ((0.0, std::f64::consts::TAU), (0.0, 10.0)),
        Surface::Nurbs { knots_u, knots_v, degree_u, degree_v, control_points, .. } => {
            let rows = control_points.len();
            let cols = control_points[0].len();
            ((knots_u[*degree_u], knots_u[rows] - 1e-14),
             (knots_v[*degree_v], knots_v[cols] - 1e-14))
        }
    }
}

/// Closest point on an analytic surface (non-NURBS, non-plane) via grid + Newton.
fn analytic_closest_point(surface: &Surface, query: DVec3) -> (f64, f64, DVec3) {
    let (u_range, v_range) = surface_param_range(surface);
    let grid = 16;
    let mut best_u = (u_range.0 + u_range.1) * 0.5;
    let mut best_v = (v_range.0 + v_range.1) * 0.5;
    let mut best_dist = f64::MAX;

    for i in 0..=grid {
        let u = u_range.0 + (u_range.1 - u_range.0) * i as f64 / grid as f64;
        for j in 0..=grid {
            let v = v_range.0 + (v_range.1 - v_range.0) * j as f64 / grid as f64;
            let pt = surface.point_at(u, v);
            let d = (pt - query).length_squared();
            if d < best_dist {
                best_dist = d;
                best_u = u;
                best_v = v;
            }
        }
    }

    // Newton refinement
    let mut u = best_u;
    let mut v = best_v;

    for _ in 0..20 {
        let pt = surface.point_at(u, v);
        let diff = pt - query;
        if diff.length() < 1e-12 { break; }

        let du = surface.partial_du(u, v);
        let dv = surface.partial_dv(u, v);

        let a11 = du.dot(du);
        let a12 = du.dot(dv);
        let a22 = dv.dot(dv);
        let b1 = -diff.dot(du);
        let b2 = -diff.dot(dv);

        let det = a11 * a22 - a12 * a12;
        if det.abs() < 1e-20 { break; }

        let delta_u = (a22 * b1 - a12 * b2) / det;
        let delta_v = (a11 * b2 - a12 * b1) / det;

        u = (u + delta_u).clamp(u_range.0, u_range.1);
        v = (v + delta_v).clamp(v_range.0, v_range.1);

        if delta_u.abs() < 1e-12 && delta_v.abs() < 1e-12 { break; }
    }

    let pt = surface.point_at(u, v);
    (u, v, pt)
}

/// Fit a NURBS curve through a sequence of 3D points using chord-length
/// parameterisation and control-point approximation.
pub fn fit_nurbs_through_points(points: &[DVec3], degree: usize) -> Curve {
    let n = points.len();
    let degree = degree.min(n - 1).max(1);

    if n <= degree + 1 {
        // Few enough points to use them directly as control points
        let m = n + degree + 1;
        let mut knots = Vec::with_capacity(m);
        for i in 0..m {
            if i <= degree {
                knots.push(0.0);
            } else if i >= m - degree - 1 {
                knots.push(1.0);
            } else {
                knots.push((i - degree) as f64 / (m - 2 * degree - 1) as f64);
            }
        }
        return Curve::Nurbs {
            control_points: points.to_vec(),
            weights: vec![1.0; n],
            knots,
            degree,
        };
    }

    // Chord-length parameterisation
    let mut params = vec![0.0_f64; n];
    let mut total_len = 0.0;
    for i in 1..n {
        total_len += (points[i] - points[i - 1]).length();
        params[i] = total_len;
    }
    if total_len > 1e-14 {
        for p in &mut params {
            *p /= total_len;
        }
    }

    // Use the points as control points for a reasonable approximation
    // For a production kernel you'd solve a linear system, but this gives
    // a usable NURBS curve for intersection results.
    let num_cp = (n / 2).max(degree + 1).min(n);
    let mut control_points = Vec::with_capacity(num_cp);

    for i in 0..num_cp {
        let t = i as f64 / (num_cp - 1) as f64;
        // Find the corresponding point by interpolation
        let idx_f = t * (n - 1) as f64;
        let idx = (idx_f as usize).min(n - 2);
        let frac = idx_f - idx as f64;
        let pt = points[idx] * (1.0 - frac) + points[idx + 1] * frac;
        control_points.push(pt);
    }

    // Ensure endpoints match exactly
    control_points[0] = points[0];
    *control_points.last_mut().unwrap() = *points.last().unwrap();

    let m = num_cp + degree + 1;
    let mut knots = Vec::with_capacity(m);
    for i in 0..m {
        if i <= degree {
            knots.push(0.0);
        } else if i >= m - degree - 1 {
            knots.push(1.0);
        } else {
            knots.push((i - degree) as f64 / (m - 2 * degree - 1) as f64);
        }
    }

    Curve::Nurbs {
        control_points,
        weights: vec![1.0; num_cp],
        knots,
        degree,
    }
}

// ---------------------------------------------------------------------------
// NURBS surface construction helpers
// ---------------------------------------------------------------------------

/// Create a NURBS surface from a grid of control points with uniform knots.
pub fn nurbs_surface_uniform(
    control_points: Vec<Vec<DVec3>>,
    degree_u: usize,
    degree_v: usize,
) -> Surface {
    let rows = control_points.len();
    let cols = control_points[0].len();
    let weights = vec![vec![1.0; cols]; rows];

    let knots_u = uniform_knot_vector(rows, degree_u);
    let knots_v = uniform_knot_vector(cols, degree_v);

    Surface::Nurbs {
        control_points,
        weights,
        knots_u,
        knots_v,
        degree_u,
        degree_v,
    }
}

/// Generate a clamped uniform knot vector for `n` control points and given degree.
pub fn uniform_knot_vector(n: usize, degree: usize) -> Vec<f64> {
    let m = n + degree + 1;
    let mut knots = Vec::with_capacity(m);
    for i in 0..m {
        if i <= degree {
            knots.push(0.0);
        } else if i >= m - degree - 1 {
            knots.push(1.0);
        } else {
            knots.push((i - degree) as f64 / (m - 2 * degree - 1) as f64);
        }
    }
    knots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_normal() {
        let s = Surface::plane(DVec3::ZERO, DVec3::Y);
        let n = s.normal_at(DVec3::new(5.0, 0.0, 3.0));
        assert!((n - DVec3::Y).length() < 1e-10);
    }

    #[test]
    fn plane_signed_distance() {
        let s = Surface::plane(DVec3::ZERO, DVec3::Y);
        assert!((s.signed_distance(DVec3::new(0.0, 3.0, 0.0)) - 3.0).abs() < 1e-10);
        assert!((s.signed_distance(DVec3::new(0.0, -2.0, 0.0)) + 2.0).abs() < 1e-10);
    }

    #[test]
    fn cylinder_normal_radial() {
        let s = Surface::cylinder(DVec3::ZERO, DVec3::Y, 5.0);
        let p = DVec3::new(5.0, 10.0, 0.0);
        let n = s.normal_at(p);
        assert!((n - DVec3::X).length() < 1e-10);
    }

    #[test]
    fn sphere_normal_outward() {
        let s = Surface::sphere(DVec3::ZERO, 3.0);
        let p = DVec3::new(3.0, 0.0, 0.0);
        let n = s.normal_at(p);
        assert!((n - DVec3::X).length() < 1e-10);
    }

    // -----------------------------------------------------------------------
    // NURBS surface evaluation tests
    // -----------------------------------------------------------------------

    /// Build a simple bilinear NURBS surface (degree 1x1, 2x2 control points).
    fn bilinear_surface() -> Surface {
        let cp = vec![
            vec![DVec3::new(0.0, 0.0, 0.0), DVec3::new(1.0, 0.0, 0.0)],
            vec![DVec3::new(0.0, 1.0, 0.0), DVec3::new(1.0, 1.0, 0.0)],
        ];
        nurbs_surface_uniform(cp, 1, 1)
    }

    #[test]
    fn nurbs_surface_point_at_corners() {
        let s = bilinear_surface();
        let p00 = s.point_at(0.0, 0.0);
        let p11 = s.point_at(1.0, 1.0);
        assert!((p00 - DVec3::ZERO).length() < 1e-6, "p00={p00:?}");
        assert!((p11 - DVec3::new(1.0, 1.0, 0.0)).length() < 1e-6, "p11={p11:?}");
    }

    #[test]
    fn nurbs_surface_point_at_center() {
        let s = bilinear_surface();
        let mid = s.point_at(0.5, 0.5);
        assert!((mid - DVec3::new(0.5, 0.5, 0.0)).length() < 1e-6);
    }

    #[test]
    fn nurbs_surface_normal_at_uv() {
        let s = bilinear_surface();
        let n = s.normal_at_uv(0.5, 0.5);
        // Flat surface in XY plane, normal should be Z (or -Z)
        assert!(n.z.abs() > 0.99, "normal should be along Z, got {n:?}");
    }

    #[test]
    fn nurbs_surface_partial_derivatives() {
        let s = bilinear_surface();
        let du = s.partial_du(0.5, 0.5);
        let dv = s.partial_dv(0.5, 0.5);
        // For bilinear surface, du and dv should be non-zero and orthogonal
        assert!(du.length() > 0.5, "du should be non-zero: {du:?}");
        assert!(dv.length() > 0.5, "dv should be non-zero: {dv:?}");
        // du and dv should lie in XY plane (z ~ 0)
        assert!(du.z.abs() < 0.01, "du z should be ~0: {du:?}");
        assert!(dv.z.abs() < 0.01, "dv z should be ~0: {dv:?}");
    }

    #[test]
    fn nurbs_surface_closest_point() {
        let s = bilinear_surface();
        // Query point above the surface center
        let query = DVec3::new(0.5, 0.5, 5.0);
        let (u, v, pt) = s.closest_point(query);
        assert!((pt - DVec3::new(0.5, 0.5, 0.0)).length() < 1e-3,
                "closest point should be (0.5, 0.5, 0): got {pt:?}");
        assert!((u - 0.5).abs() < 0.1, "u should be ~0.5: got {u}");
        assert!((v - 0.5).abs() < 0.1, "v should be ~0.5: got {v}");
    }

    #[test]
    fn nurbs_surface_closest_point_off_center() {
        let s = bilinear_surface();
        let query = DVec3::new(0.8, 0.2, 3.0);
        let (_, _, pt) = s.closest_point(query);
        assert!((pt.x - 0.8).abs() < 0.05);
        assert!((pt.y - 0.2).abs() < 0.05);
        assert!(pt.z.abs() < 0.01);
    }

    #[test]
    fn sphere_closest_point() {
        let s = Surface::sphere(DVec3::ZERO, 5.0);
        let query = DVec3::new(10.0, 0.0, 0.0);
        let (_, _, pt) = s.closest_point(query);
        assert!((pt - DVec3::new(5.0, 0.0, 0.0)).length() < 0.2,
                "closest point on sphere should be (5,0,0): got {pt:?}");
    }

    #[test]
    fn surface_intersection_plane_sphere() {
        // A plane cutting through a sphere should produce intersection curve(s)
        let sphere = Surface::sphere(DVec3::ZERO, 5.0);
        let plane = Surface::plane(DVec3::new(0.0, 0.0, 2.0), DVec3::Z);
        let curves = intersect_surfaces(&sphere, &plane, 0.5);
        // The intersection of a sphere with a plane is a circle (one curve)
        assert!(!curves.is_empty(), "should find at least one intersection curve");
    }

    #[test]
    fn surface_intersection_no_contact() {
        // Two planes that are parallel and not coincident
        let p1 = Surface::plane(DVec3::ZERO, DVec3::Z);
        let p2 = Surface::plane(DVec3::new(0.0, 0.0, 100.0), DVec3::Z);
        let curves = intersect_surfaces(&p1, &p2, 0.1);
        assert!(curves.is_empty(), "parallel planes should not intersect");
    }

    #[test]
    fn fit_nurbs_basic() {
        let pts = vec![
            DVec3::ZERO,
            DVec3::new(1.0, 1.0, 0.0),
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(3.0, 1.0, 0.0),
        ];
        let curve = fit_nurbs_through_points(&pts, 3);
        let start = curve.evaluate(0.0);
        let end = curve.evaluate(1.0);
        assert!((start - pts[0]).length() < 0.1, "start={start:?}");
        assert!((end - *pts.last().unwrap()).length() < 0.1, "end={end:?}");
    }

    #[test]
    fn uniform_knot_vector_structure() {
        let k = uniform_knot_vector(5, 3);
        assert_eq!(k.len(), 9); // 5 + 3 + 1
        assert_eq!(k[0], 0.0);
        assert_eq!(*k.last().unwrap(), 1.0);
    }
}
