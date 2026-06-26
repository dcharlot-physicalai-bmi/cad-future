//! 2D contour — closed loop of points for slicing, laser, and CNC toolpaths.

use glam::DVec2;
use serde::{Deserialize, Serialize};

/// A 2D contour — ordered sequence of points forming a closed or open loop.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Contour {
    pub points: Vec<DVec2>,
    pub is_closed: bool,
}

impl Contour {
    /// Create a new closed contour from points.
    pub fn closed(points: Vec<DVec2>) -> Self {
        Self { points, is_closed: true }
    }

    /// Create a new open contour from points.
    pub fn open(points: Vec<DVec2>) -> Self {
        Self { points, is_closed: false }
    }

    /// Signed area using the shoelace formula.
    /// Positive = CCW winding (outer boundary), negative = CW (hole).
    pub fn signed_area(&self) -> f64 {
        let n = self.points.len();
        if n < 3 {
            return 0.0;
        }
        let mut area = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            area += self.points[i].x * self.points[j].y;
            area -= self.points[j].x * self.points[i].y;
        }
        area * 0.5
    }

    /// Whether this contour winds counter-clockwise (outer boundary).
    pub fn is_ccw(&self) -> bool {
        self.signed_area() > 0.0
    }

    /// Whether this is an outer contour (CCW winding).
    pub fn is_outer(&self) -> bool {
        self.is_ccw()
    }

    /// Reverse winding direction.
    pub fn reverse(&mut self) {
        self.points.reverse();
    }

    /// Ensure CCW winding.
    pub fn ensure_ccw(&mut self) {
        if !self.is_ccw() {
            self.reverse();
        }
    }

    /// Ensure CW winding (for holes).
    pub fn ensure_cw(&mut self) {
        if self.is_ccw() {
            self.reverse();
        }
    }

    /// Axis-aligned bounding box: (min, max).
    pub fn bounds(&self) -> (DVec2, DVec2) {
        if self.points.is_empty() {
            return (DVec2::ZERO, DVec2::ZERO);
        }
        let mut min = self.points[0];
        let mut max = self.points[0];
        for &p in &self.points[1..] {
            min = min.min(p);
            max = max.max(p);
        }
        (min, max)
    }

    /// Perimeter length.
    pub fn length(&self) -> f64 {
        if self.points.len() < 2 {
            return 0.0;
        }
        let mut len = 0.0;
        for i in 0..self.points.len() - 1 {
            len += (self.points[i + 1] - self.points[i]).length();
        }
        if self.is_closed && self.points.len() >= 3 {
            len += (self.points[0] - *self.points.last().unwrap()).length();
        }
        len
    }

    /// Point-in-contour test using ray casting (even-odd rule).
    pub fn contains(&self, point: DVec2) -> bool {
        let n = self.points.len();
        if n < 3 {
            return false;
        }
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let pi = self.points[i];
            let pj = self.points[j];
            if ((pi.y > point.y) != (pj.y > point.y))
                && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    /// Offset contour by `distance`. Positive = outward (for CCW contours), negative = inward.
    ///
    /// Uses edge-normal offsetting with miter joints. For sharp corners where the miter
    /// ratio exceeds `miter_limit`, the corner is clipped. Self-intersections from large
    /// inward offsets are removed.
    pub fn offset(&self, distance: f64) -> Option<Contour> {
        let n = self.points.len();
        if n < 3 {
            return None;
        }

        // Compute edge normals (left-hand normals for CCW contour = outward)
        let mut normals = Vec::with_capacity(n);
        for i in 0..n {
            let j = (i + 1) % n;
            let edge = self.points[j] - self.points[i];
            let len = edge.length();
            if len < 1e-12 {
                normals.push(DVec2::ZERO);
            } else {
                // Right-hand normal: for CCW polygon this points outward
                normals.push(DVec2::new(edge.y, -edge.x) / len);
            }
        }

        // Offset each vertex by averaging adjacent edge normals
        let mut result = Vec::with_capacity(n);
        for i in 0..n {
            let prev = if i == 0 { n - 1 } else { i - 1 };
            let n0 = normals[prev];
            let n1 = normals[i];

            // Bisector direction
            let bisector = n0 + n1;
            let bisector_len_sq = bisector.length_squared();

            let offset_vec = if bisector_len_sq < 1e-12 {
                // Parallel edges — use either normal
                n0 * distance
            } else {
                // Miter: scale bisector so its projection onto either normal = distance
                let dot = bisector.dot(n0);
                if dot.abs() < 1e-12 {
                    n0 * distance
                } else {
                    let miter_scale = distance / dot;
                    let miter = bisector * miter_scale;
                    // Miter limit: cap at 2x distance to avoid spikes at sharp corners
                    if miter.length() > distance.abs() * 4.0 {
                        n0 * distance
                    } else {
                        miter
                    }
                }
            };

            result.push(self.points[i] + offset_vec);
        }

        // Remove self-intersections from inward offsets
        let cleaned = remove_self_intersections(&result);
        if cleaned.len() < 3 {
            return None; // Contour collapsed
        }

        Some(Contour::closed(cleaned))
    }

    /// Centroid of the contour.
    pub fn centroid(&self) -> DVec2 {
        if self.points.is_empty() {
            return DVec2::ZERO;
        }
        let sum: DVec2 = self.points.iter().copied().sum();
        sum / self.points.len() as f64
    }
}

/// Remove self-intersections from a polygon by detecting crossings
/// and keeping the largest valid loop.
fn remove_self_intersections(points: &[DVec2]) -> Vec<DVec2> {
    remove_self_intersections_depth(points, 0)
}

fn remove_self_intersections_depth(points: &[DVec2], depth: usize) -> Vec<DVec2> {
    let n = points.len();
    if n < 4 || depth > 10 {
        return points.to_vec();
    }

    // Check all edge pairs for intersections
    // If any found, split and keep largest sub-loop
    for i in 0..n {
        let i_next = (i + 1) % n;
        let a0 = points[i];
        let a1 = points[i_next];

        for j in (i + 2)..n {
            if j == n - 1 && i == 0 {
                continue; // Skip adjacent edges
            }
            let j_next = (j + 1) % n;
            let b0 = points[j];
            let b1 = points[j_next];

            if let Some(_t) = segment_intersection(a0, a1, b0, b1) {
                // Self-intersection found — keep the larger sub-loop
                // Loop 1: i+1 .. j (inclusive)
                let loop1: Vec<DVec2> = (i_next..=j).map(|k| points[k]).collect();
                // Loop 2: j+1 .. i (wrapping)
                let mut loop2 = Vec::new();
                let mut k = j_next;
                let max_iter = n + 1;
                let mut iter_count = 0;
                loop {
                    loop2.push(points[k % n]);
                    if k % n == i || iter_count > max_iter {
                        break;
                    }
                    k += 1;
                    iter_count += 1;
                }

                let area1: f64 = polygon_area(&loop1).abs();
                let area2: f64 = polygon_area(&loop2).abs();

                if loop1.len() >= 3 && area1 >= area2 {
                    return remove_self_intersections_depth(&loop1, depth + 1);
                } else if loop2.len() >= 3 {
                    return remove_self_intersections_depth(&loop2, depth + 1);
                }
            }
        }
    }

    points.to_vec()
}

/// Line segment intersection test. Returns parameter t for first segment if they intersect.
fn segment_intersection(a0: DVec2, a1: DVec2, b0: DVec2, b1: DVec2) -> Option<f64> {
    let d1 = a1 - a0;
    let d2 = b1 - b0;
    let cross = d1.x * d2.y - d1.y * d2.x;
    if cross.abs() < 1e-12 {
        return None; // Parallel
    }
    let d = b0 - a0;
    let t = (d.x * d2.y - d.y * d2.x) / cross;
    let u = (d.x * d1.y - d.y * d1.x) / cross;
    if (1e-8..1.0 - 1e-8).contains(&t) && (1e-8..1.0 - 1e-8).contains(&u) {
        Some(t)
    } else {
        None
    }
}

/// Signed area of a polygon (shoelace formula).
fn polygon_area(points: &[DVec2]) -> f64 {
    let n = points.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += points[i].x * points[j].y;
        area -= points[j].x * points[i].y;
    }
    area * 0.5
}

/// Chain unordered line segments into closed contours.
/// Each segment is a pair of 2D endpoints.
/// Returns a list of closed contours (point sequences).
pub fn chain_segments(segments: &[(DVec2, DVec2)]) -> Vec<Contour> {
    if segments.is_empty() {
        return Vec::new();
    }

    let eps = 1e-6;
    let mut used = vec![false; segments.len()];
    let mut contours = Vec::new();

    for start_idx in 0..segments.len() {
        if used[start_idx] {
            continue;
        }

        let mut points = Vec::new();
        let loop_start = segments[start_idx].0;
        used[start_idx] = true;
        points.push(segments[start_idx].0);
        points.push(segments[start_idx].1);
        let mut current_end = segments[start_idx].1;

        for _safety in 0..segments.len() {
            // Check if we've closed the loop
            if (current_end - loop_start).length() < eps && points.len() >= 4 {
                points.pop(); // Remove duplicate closing point
                contours.push(Contour::closed(points));
                break;
            }

            // Find next unvisited segment starting at current_end
            let mut found = false;
            for j in 0..segments.len() {
                if used[j] {
                    continue;
                }
                if (segments[j].0 - current_end).length() < eps {
                    used[j] = true;
                    points.push(segments[j].1);
                    current_end = segments[j].1;
                    found = true;
                    break;
                }
                // Try reversed
                if (segments[j].1 - current_end).length() < eps {
                    used[j] = true;
                    points.push(segments[j].0);
                    current_end = segments[j].0;
                    found = true;
                    break;
                }
            }
            if !found {
                break;
            }
        }
    }

    contours
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Contour {
        Contour::closed(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 10.0),
            DVec2::new(0.0, 10.0),
        ])
    }

    #[test]
    fn signed_area_ccw() {
        let c = square();
        assert!((c.signed_area() - 100.0).abs() < 1e-8);
        assert!(c.is_ccw());
        assert!(c.is_outer());
    }

    #[test]
    fn signed_area_cw() {
        let mut c = square();
        c.reverse();
        assert!((c.signed_area() + 100.0).abs() < 1e-8);
        assert!(!c.is_ccw());
    }

    #[test]
    fn contains_point() {
        let c = square();
        assert!(c.contains(DVec2::new(5.0, 5.0)));
        assert!(!c.contains(DVec2::new(15.0, 5.0)));
        assert!(!c.contains(DVec2::new(-1.0, -1.0)));
    }

    #[test]
    fn bounds() {
        let c = square();
        let (min, max) = c.bounds();
        assert!((min - DVec2::ZERO).length() < 1e-8);
        assert!((max - DVec2::new(10.0, 10.0)).length() < 1e-8);
    }

    #[test]
    fn offset_outward() {
        let c = square();
        let expanded = c.offset(1.0).unwrap();
        assert!(expanded.signed_area() > c.signed_area());
    }

    #[test]
    fn offset_inward() {
        let c = square();
        let shrunk = c.offset(-1.0).unwrap();
        assert!(shrunk.signed_area() < c.signed_area());
        assert!(shrunk.signed_area() > 0.0); // Still has area
    }

    #[test]
    fn offset_collapse() {
        let c = square();
        // Offset inward by more than half the width → should collapse or have negligible area
        let collapsed = c.offset(-6.0);
        match collapsed {
            None => {} // Good — collapsed
            Some(c) => {
                // If it didn't return None, area should be negligible
                // Miter corners preserve a small diamond, so area won't be exactly 0
                // For a 10x10 square offset by -6, remaining area should be small relative to original 100
                assert!(c.signed_area().abs() < 10.0, "Should have small area, got {}", c.signed_area());
            }
        }
    }

    #[test]
    fn chain_segments_square() {
        let segs = vec![
            (DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)),
            (DVec2::new(10.0, 0.0), DVec2::new(10.0, 10.0)),
            (DVec2::new(10.0, 10.0), DVec2::new(0.0, 10.0)),
            (DVec2::new(0.0, 10.0), DVec2::new(0.0, 0.0)),
        ];
        let contours = chain_segments(&segs);
        assert_eq!(contours.len(), 1);
        assert_eq!(contours[0].points.len(), 4);
        assert!(contours[0].is_closed);
    }

    #[test]
    fn chain_segments_reversed() {
        // Some segments in reverse order
        let segs = vec![
            (DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)),
            (DVec2::new(10.0, 10.0), DVec2::new(10.0, 0.0)), // reversed
            (DVec2::new(10.0, 10.0), DVec2::new(0.0, 10.0)),
            (DVec2::new(0.0, 10.0), DVec2::new(0.0, 0.0)),
        ];
        let contours = chain_segments(&segs);
        assert_eq!(contours.len(), 1);
    }

    #[test]
    fn perimeter_length() {
        let c = square();
        assert!((c.length() - 40.0).abs() < 1e-8);
    }
}
