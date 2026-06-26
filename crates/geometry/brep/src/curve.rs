//! Curve types for B-Rep edges.

use glam::DVec3;
use serde::{Serialize, Deserialize};

/// A 3D curve used as the geometric support for an edge.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Curve {
    /// Straight line between two points.
    Line { start: DVec3, end: DVec3 },
    /// Circular arc.
    Arc {
        center: DVec3,
        axis: DVec3,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
    /// Full circle.
    Circle {
        center: DVec3,
        axis: DVec3,
        radius: f64,
    },
    /// Non-uniform rational B-spline curve.
    Nurbs {
        /// Control points (weighted: multiply by weight before storing).
        control_points: Vec<DVec3>,
        /// Weights for each control point (1.0 = non-rational).
        weights: Vec<f64>,
        /// Knot vector (length = control_points.len() + degree + 1).
        knots: Vec<f64>,
        /// Polynomial degree.
        degree: usize,
    },
}

impl Curve {
    pub fn line(start: DVec3, end: DVec3) -> Self {
        Self::Line { start, end }
    }

    pub fn arc(center: DVec3, axis: DVec3, radius: f64, start: f64, end: f64) -> Self {
        Self::Arc { center, axis, radius, start_angle: start, end_angle: end }
    }

    pub fn circle(center: DVec3, axis: DVec3, radius: f64) -> Self {
        Self::Circle { center, axis, radius }
    }

    /// Evaluate point on curve at parameter t in [0, 1].
    pub fn evaluate(&self, t: f64) -> DVec3 {
        match self {
            Self::Line { start, end } => *start + (*end - *start) * t,
            Self::Arc { center, axis, radius, start_angle, end_angle } => {
                let angle = start_angle + (end_angle - start_angle) * t;
                let (u, v) = perpendicular_frame(*axis);
                *center + u * (radius * angle.cos()) + v * (radius * angle.sin())
            }
            Self::Circle { center, axis, radius } => {
                let angle = std::f64::consts::TAU * t;
                let (u, v) = perpendicular_frame(*axis);
                *center + u * (radius * angle.cos()) + v * (radius * angle.sin())
            }
            Self::Nurbs { control_points, weights, knots, degree } => {
                // Map t from [0,1] to knot domain
                let u = knots[*degree] + t * (knots[knots.len() - degree - 1] - knots[*degree]);
                nurbs_curve_point(control_points, weights, knots, *degree, u)
            }
        }
    }

    /// Approximate curve length.
    pub fn length(&self) -> f64 {
        match self {
            Self::Line { start, end } => (*end - *start).length(),
            Self::Arc { radius, start_angle, end_angle, .. } => {
                radius * (end_angle - start_angle).abs()
            }
            Self::Circle { radius, .. } => std::f64::consts::TAU * radius,
            Self::Nurbs { .. } => {
                // Approximate by sampling
                let n = 32;
                let mut len = 0.0;
                let mut prev = self.evaluate(0.0);
                for i in 1..=n {
                    let t = i as f64 / n as f64;
                    let p = self.evaluate(t);
                    len += (p - prev).length();
                    prev = p;
                }
                len
            }
        }
    }

    /// Midpoint of the curve.
    pub fn midpoint(&self) -> DVec3 {
        self.evaluate(0.5)
    }
}

// ---------------------------------------------------------------------------
// NURBS evaluation (Cox-de Boor algorithm)
// ---------------------------------------------------------------------------

/// Evaluate a B-spline basis function N_{i,p}(u) using Cox-de Boor recursion.
pub fn bspline_basis(i: usize, p: usize, u: f64, knots: &[f64]) -> f64 {
    if p == 0 {
        return if knots[i] <= u && u < knots[i + 1] { 1.0 } else { 0.0 };
    }

    let mut left = 0.0;
    let denom_left = knots[i + p] - knots[i];
    if denom_left > 1e-14 {
        left = (u - knots[i]) / denom_left * bspline_basis(i, p - 1, u, knots);
    }

    let mut right = 0.0;
    let denom_right = knots[i + p + 1] - knots[i + 1];
    if denom_right > 1e-14 {
        right = (knots[i + p + 1] - u) / denom_right * bspline_basis(i + 1, p - 1, u, knots);
    }

    left + right
}

/// Evaluate a NURBS curve point at parameter u.
pub fn nurbs_curve_point(
    control_points: &[DVec3],
    weights: &[f64],
    knots: &[f64],
    degree: usize,
    u: f64,
) -> DVec3 {
    let n = control_points.len();
    let mut numerator = DVec3::ZERO;
    let mut denominator = 0.0;

    // Clamp u to avoid evaluating outside the valid knot span
    let u = u.clamp(knots[degree], knots[n] - 1e-14);

    for i in 0..n {
        let basis = bspline_basis(i, degree, u, knots);
        let w = weights[i];
        numerator += control_points[i] * basis * w;
        denominator += basis * w;
    }

    if denominator.abs() > 1e-14 { numerator / denominator } else { control_points[0] }
}

/// Create a NURBS curve from control points with uniform knots.
pub fn nurbs_uniform(control_points: Vec<DVec3>, degree: usize) -> Curve {
    let n = control_points.len();
    let weights = vec![1.0; n];
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
    Curve::Nurbs { control_points, weights, knots, degree }
}

/// Build an orthonormal frame perpendicular to `axis`.
pub fn perpendicular_frame(axis: DVec3) -> (DVec3, DVec3) {
    let a = axis.normalize();
    let up = if a.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
    let u = a.cross(up).normalize();
    let v = a.cross(u).normalize();
    (u, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_endpoints() {
        let c = Curve::line(DVec3::ZERO, DVec3::X * 10.0);
        assert!((c.evaluate(0.0) - DVec3::ZERO).length() < 1e-10);
        assert!((c.evaluate(1.0) - DVec3::X * 10.0).length() < 1e-10);
    }

    #[test]
    fn line_midpoint() {
        let c = Curve::line(DVec3::ZERO, DVec3::new(4.0, 0.0, 0.0));
        assert!((c.midpoint() - DVec3::new(2.0, 0.0, 0.0)).length() < 1e-10);
    }

    #[test]
    fn line_length() {
        let c = Curve::line(DVec3::ZERO, DVec3::new(3.0, 4.0, 0.0));
        assert!((c.length() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn circle_length() {
        let c = Curve::circle(DVec3::ZERO, DVec3::Z, 1.0);
        assert!((c.length() - std::f64::consts::TAU).abs() < 1e-10);
    }

    #[test]
    fn perpendicular_frame_orthogonal() {
        let (u, v) = perpendicular_frame(DVec3::Z);
        assert!(u.dot(v).abs() < 1e-10);
        assert!(u.dot(DVec3::Z).abs() < 1e-10);
        assert!(v.dot(DVec3::Z).abs() < 1e-10);
    }

    #[test]
    fn nurbs_line_equivalent() {
        // Degree-1 NURBS with 2 control points = straight line
        let c = Curve::Nurbs {
            control_points: vec![DVec3::ZERO, DVec3::new(10.0, 0.0, 0.0)],
            weights: vec![1.0, 1.0],
            knots: vec![0.0, 0.0, 1.0, 1.0],
            degree: 1,
        };
        let mid = c.evaluate(0.5);
        assert!((mid - DVec3::new(5.0, 0.0, 0.0)).length() < 1e-6);
        assert!((c.evaluate(0.0) - DVec3::ZERO).length() < 1e-6);
    }

    #[test]
    fn nurbs_quadratic_curve() {
        // Degree-2 NURBS: 3 control points forming a parabola
        let c = Curve::Nurbs {
            control_points: vec![
                DVec3::ZERO,
                DVec3::new(5.0, 10.0, 0.0),
                DVec3::new(10.0, 0.0, 0.0),
            ],
            weights: vec![1.0, 1.0, 1.0],
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            degree: 2,
        };
        let mid = c.evaluate(0.5);
        assert!((mid.x - 5.0).abs() < 0.1, "x={}", mid.x);
        assert!(mid.y > 0.0, "curve should arch up");
    }

    #[test]
    fn nurbs_uniform_constructor() {
        let c = nurbs_uniform(vec![
            DVec3::ZERO,
            DVec3::new(3.0, 5.0, 0.0),
            DVec3::new(7.0, 5.0, 0.0),
            DVec3::new(10.0, 0.0, 0.0),
        ], 3);
        match &c {
            Curve::Nurbs { knots, degree, .. } => {
                assert_eq!(*degree, 3);
                assert_eq!(knots.len(), 8); // 4 + 3 + 1
            }
            _ => panic!("expected Nurbs"),
        }
        // Should interpolate endpoints
        assert!((c.evaluate(0.0) - DVec3::ZERO).length() < 1e-6);
    }

    #[test]
    fn nurbs_length_positive() {
        let c = nurbs_uniform(vec![
            DVec3::ZERO,
            DVec3::new(5.0, 5.0, 0.0),
            DVec3::new(10.0, 0.0, 0.0),
        ], 2);
        assert!(c.length() > 10.0, "curved path should be longer than chord");
    }

    #[test]
    fn nurbs_weighted_pulls_toward_control_point() {
        // High weight on middle control point should pull curve toward it
        let c_uniform = Curve::Nurbs {
            control_points: vec![
                DVec3::ZERO,
                DVec3::new(5.0, 10.0, 0.0),
                DVec3::new(10.0, 0.0, 0.0),
            ],
            weights: vec![1.0, 1.0, 1.0],
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            degree: 2,
        };
        let c_heavy = Curve::Nurbs {
            control_points: vec![
                DVec3::ZERO,
                DVec3::new(5.0, 10.0, 0.0),
                DVec3::new(10.0, 0.0, 0.0),
            ],
            weights: vec![1.0, 10.0, 1.0],
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            degree: 2,
        };
        let mid_uniform = c_uniform.evaluate(0.5);
        let mid_heavy = c_heavy.evaluate(0.5);
        assert!(mid_heavy.y > mid_uniform.y, "heavy weight should pull curve up");
    }
}
